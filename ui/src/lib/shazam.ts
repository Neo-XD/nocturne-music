import * as api from './api';
import type { RecognizedSong } from './api';

// CRC32 table for Shazam packet integrity
const CRC_TABLE = new Uint32Array(256);
for (let i = 0; i < 256; i++) {
	let c = i;
	for (let k = 0; k < 8; k++) {
		c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
	}
	CRC_TABLE[i] = c >>> 0;
}

function computeCrc32(bytes: Uint8Array, start = 0, length = bytes.length - start): number {
	let crc = 0 ^ -1;
	for (let i = start; i < start + length; i++) {
		crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ bytes[i]) & 0xff];
	}
	return (crc ^ -1) >>> 0;
}

// 2048-point Radix-2 in-place Cooley-Tukey FFT
function fft2048(real: Float32Array, imag: Float32Array) {
	const n = 2048;
	let j = 0;
	for (let i = 0; i < n - 1; i++) {
		if (i < j) {
			const tr = real[i];
			real[i] = real[j];
			real[j] = tr;
			const ti = imag[i];
			imag[i] = imag[j];
			imag[j] = ti;
		}
		let k = n >> 1;
		while (k <= j) {
			j -= k;
			k >>= 1;
		}
		j += k;
	}

	for (let len = 2; len <= n; len <<= 1) {
		const half = len >> 1;
		const angle = (-2 * Math.PI) / len;
		const wstepR = Math.cos(angle);
		const wstepI = Math.sin(angle);
		let wR = 1;
		let wI = 0;

		for (let m = 0; m < half; m++) {
			for (let i = m; i < n; i += len) {
				const pos = i + half;
				const tr = wR * real[pos] - wI * imag[pos];
				const ti = wR * imag[pos] + wI * real[pos];
				real[pos] = real[i] - tr;
				imag[pos] = imag[i] - ti;
				real[i] += tr;
				imag[i] += ti;
			}
			const nextR = wR * wstepR - wI * wstepI;
			wI = wR * wstepI + wI * wstepR;
			wR = nextR;
		}
	}
}

interface Peak {
	fftPassNumber: number;
	peakMagnitude: number;
	correctedPeakFrequencyBin: number;
}

const BANDS = [
	{ minBin: 32, maxBin: 66 },    // 250 Hz - 520 Hz
	{ minBin: 67, maxBin: 185 },   // 520 Hz - 1450 Hz
	{ minBin: 186, maxBin: 448 },  // 1450 Hz - 3500 Hz
	{ minBin: 449, maxBin: 704 }   // 3500 Hz - 5500 Hz
];

// Precompute Hanning window 2048 multipliers
const HANNING = new Float32Array(2048);
for (let i = 0; i < 2048; i++) {
	HANNING[i] = 0.5 * (1 - Math.cos((2 * Math.PI * i) / 2048));
}

export function generateShazamUriFromPcm(samples: Float32Array, sampleRate = 16000): string {
	const step = 128;
	const frameSize = 2048;
	const numPasses = Math.floor((samples.length - frameSize) / step);

	const bandPeaks: Peak[][] = [[], [], [], []];

	const real = new Float32Array(frameSize);
	const imag = new Float32Array(frameSize);
	const magnitudes = new Float32Array(1025);

	for (let pass = 0; pass < numPasses; pass++) {
		const offset = pass * step;
		for (let i = 0; i < frameSize; i++) {
			real[i] = samples[offset + i] * HANNING[i];
			imag[i] = 0;
		}

		fft2048(real, imag);

		for (let i = 0; i < 1025; i++) {
			magnitudes[i] = Math.sqrt(real[i] * real[i] + imag[i] * imag[i]);
		}

		// Find local peaks in each frequency band
		for (let b = 0; b < 4; b++) {
			const { minBin, maxBin } = BANDS[b];
			let maxMag = 0;
			let maxBinIdx = minBin;

			for (let bin = minBin; bin <= maxBin; bin++) {
				const m = magnitudes[bin];
				if (m > maxMag && m > magnitudes[bin - 1] && m > magnitudes[bin + 1]) {
					maxMag = m;
					maxBinIdx = bin;
				}
			}

			if (maxMag > 0.05) {
				// Normalize magnitude to Shazam u16 range (log-scaled)
				const scaledMag = Math.min(65535, Math.floor(Math.log1p(maxMag * 50) * 5000));
				bandPeaks[b].push({
					fftPassNumber: pass,
					peakMagnitude: scaledMag,
					correctedPeakFrequencyBin: maxBinIdx * 64
				});
			}
		}
	}

	// Serialize binary format
	// Calculate size: Header (48 bytes) + TLV Container (8 bytes) + Band chunks
	const bandBuffers: Uint8Array[] = [];

	for (let b = 0; b < 4; b++) {
		const peaks = bandPeaks[b];
		if (!peaks.length) continue;

		const bytes: number[] = [];
		let lastPass = 0;

		for (const peak of peaks) {
			const diff = peak.fftPassNumber - lastPass;
			if (diff >= 255) {
				bytes.push(0xff);
				// Little endian u32 pass number
				bytes.push(peak.fftPassNumber & 0xff);
				bytes.push((peak.fftPassNumber >> 8) & 0xff);
				bytes.push((peak.fftPassNumber >> 16) & 0xff);
				bytes.push((peak.fftPassNumber >> 24) & 0xff);
			} else {
				bytes.push(diff & 0xff);
			}

			// u16 Little endian magnitude
			bytes.push(peak.peakMagnitude & 0xff);
			bytes.push((peak.peakMagnitude >> 8) & 0xff);

			// u16 Little endian corrected bin
			bytes.push(peak.correctedPeakFrequencyBin & 0xff);
			bytes.push((peak.correctedPeakFrequencyBin >> 8) & 0xff);

			lastPass = peak.fftPassNumber;
		}

		// Chunk header: ID (4 bytes) + Size (4 bytes) + Content + padding to 4 bytes
		const padding = (4 - (bytes.length % 4)) % 4;
		const chunkBuf = new Uint8Array(8 + bytes.length + padding);
		const view = new DataView(chunkBuf.buffer);

		view.setUint32(0, 0x60030040 + b, true);
		view.setUint32(4, bytes.length, true);
		chunkBuf.set(bytes, 8);
		bandBuffers.push(chunkBuf);
	}

	let totalBandBytes = 0;
	for (const buf of bandBuffers) totalBandBytes += buf.length;

	const totalSize = 48 + 8 + totalBandBytes;
	const out = new Uint8Array(totalSize);
	const view = new DataView(out.buffer);

	// Header
	view.setUint32(0, 0xcafe2580, true);
	view.setUint32(4, 0, true); // CRC placeholder
	view.setUint32(8, totalSize - 48, true);
	view.setUint32(12, 0x94119c00, true);
	view.setUint32(16, 0, true);
	view.setUint32(20, 0, true);
	view.setUint32(24, 0, true);
	view.setUint32(28, 3 << 27, true); // 16000 Hz
	view.setUint32(32, 0, true);
	view.setUint32(36, 0, true);
	view.setUint32(40, samples.length + Math.floor(sampleRate * 0.24), true);
	view.setUint32(44, (15 << 19) + 0x40000, true);

	// Container chunk
	view.setUint32(48, 0x40000000, true);
	view.setUint32(52, totalSize - 48, true);

	let cursor = 56;
	for (const buf of bandBuffers) {
		out.set(buf, cursor);
		cursor += buf.length;
	}

	// Compute CRC-32 over bytes starting at index 8
	const crc = computeCrc32(out, 8, totalSize - 8);
	view.setUint32(4, crc, true);

	// Base64 encoding
	let binaryString = '';
	for (let i = 0; i < out.length; i++) {
		binaryString += String.fromCharCode(out[i]);
	}
	const base64 = btoa(binaryString);

	return `data:audio/vnd.shazam.sig;base64,${base64}`;
}

export async function captureAndRecognizeAudio(
	onProgress?: (state: string) => void
): Promise<RecognizedSong> {
	onProgress?.('Listening to PC audio…');

	// Attempt direct system audio loopback capture without needing a microphone
	try {
		const pcSamples = await api.capturePcAudio(4500);
		if (pcSamples && pcSamples.length > 0) {
			onProgress?.('Identifying song with Shazam…');
			const recorded = new Float32Array(pcSamples);
			const uri = generateShazamUriFromPcm(recorded, 16000);
			const durationMs = Math.round((recorded.length / 16000) * 1000);
			return await api.recognizeSongSignature(uri, durationMs);
		}
	} catch (err) {
		console.warn('Direct PC loopback capture failed or unavailable, falling back to mic:', err);
	}

	if (typeof navigator === 'undefined' || !navigator.mediaDevices) {
		throw new Error('Microphone and PC audio capture not supported in this environment');
	}

	// Request capture: try standard audio input (microphone or Stereo Mix / loopback)
	const stream = await navigator.mediaDevices.getUserMedia({
		audio: {
			echoCancellation: false,
			noiseSuppression: false,
			autoGainControl: false
		}
	});

	try {
		const audioCtx = new (window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)({
			sampleRate: 16000
		});

		const source = audioCtx.createMediaStreamSource(stream);
		const processor = audioCtx.createScriptProcessor(4096, 1, 1);

		const targetSamples = 16000 * 4.5; // 4.5 seconds
		const recorded = new Float32Array(targetSamples);
		let sampleCount = 0;

		await new Promise<void>((resolve) => {
			processor.onaudioprocess = (e) => {
				const input = e.inputBuffer.getChannelData(0);
				for (let i = 0; i < input.length; i++) {
					if (sampleCount < targetSamples) {
						recorded[sampleCount++] = input[i];
					} else {
						resolve();
						return;
					}
				}
			};

			source.connect(processor);
			processor.connect(audioCtx.destination);
		});

		// Clean up audio nodes
		processor.disconnect();
		source.disconnect();
		await audioCtx.close();

		onProgress?.('Identifying song with Shazam…');

		const uri = generateShazamUriFromPcm(recorded, 16000);
		const result = await api.recognizeSongSignature(uri, 4500);

		return result;
	} finally {
		// Stop all tracks in the stream
		stream.getTracks().forEach((t) => t.stop());
	}
}
