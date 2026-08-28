<script lang="ts">
	import { onMount } from 'svelte';
	import { playback } from '$lib/player.svelte';

	let {
		src,
		alt = '',
		class: className = '',
		intensity = 1.0,
		speed = 1.0
	}: {
		src?: string | null;
		alt?: string;
		class?: string;
		intensity?: number;
		speed?: number;
	} = $props();

	let canvasEl: HTMLCanvasElement | undefined = $state();
	let gl: WebGLRenderingContext | null = null;
	let program: WebGLProgram | null = null;
	let currentTexture: WebGLTexture | null = null;
	let nextTexture: WebGLTexture | null = null;
	let textureMix = 1.0;
	let currentSrc = '';
	let animId = 0;
	let startTime = performance.now();
	let pausedAt = 0;
	let totalPausedDuration = 0;
	let isVisible = true;
	let webglFailed = $state(false);

	const VS_SOURCE = `
		attribute vec2 a_position;
		varying vec2 v_uv;
		void main() {
			v_uv = (a_position + 1.0) * 0.5;
			v_uv.y = 1.0 - v_uv.y;
			gl_Position = vec4(a_position, 0.0, 1.0);
		}
	`;

	const FS_SOURCE = `
		precision highp float;
		varying vec2 v_uv;
		uniform sampler2D u_image;
		uniform sampler2D u_next_image;
		uniform float u_mix;
		uniform float u_time;
		uniform float u_speed;
		uniform float u_intensity;

		// 2D Simplex Noise for organic fluid domain warping
		vec3 mod289(vec3 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
		vec2 mod289(vec2 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
		vec3 permute(vec3 x) { return mod289(((x * 34.0) + 1.0) * x); }

		float snoise(vec2 v) {
			const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
			vec2 i  = floor(v + dot(v, C.yy));
			vec2 x0 = v - i + dot(i, C.xx);
			vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
			vec4 x12 = x0.xyxy + C.xxzz;
			x12.xy -= i1;
			i = mod289(i);
			vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
			vec3 m = max(0.5 - vec3(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), 0.0);
			m = m * m;
			m = m * m;
			vec3 x = 2.0 * fract(p * C.www) - 1.0;
			vec3 h = abs(x) - 0.5;
			vec3 ox = floor(x + 0.5);
			vec3 a0 = x - ox;
			m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
			vec3 g;
			g.x  = a0.x  * x0.x  + h.x  * x0.y;
			g.yz = a0.yz * x12.xz + h.yz * x12.yw;
			return 130.0 * dot(m, g);
		}

		void main() {
			vec2 uv = v_uv;
			float t = u_time * u_speed * 0.45;

			// Dual-octave domain warping (Kawarp style)
			vec2 q = vec2(
				snoise(uv * 2.0 + vec2(t * 0.35, t * 0.25)),
				snoise(uv * 2.0 + vec2(t * 0.25 + 4.3, t * 0.35 + 2.1))
			);

			vec2 r = vec2(
				snoise(uv * 2.8 + 4.0 * q + vec2(t * 0.4 + 1.7, t * 0.45 + 9.2)),
				snoise(uv * 2.8 + 4.0 * q + vec2(t * 0.35 + 8.3, t * 0.4 + 2.8))
			);

			// Fluid offset with boundary clamp
			vec2 warpedUv = uv + r * (0.045 * u_intensity);
			warpedUv = clamp(warpedUv, 0.002, 0.998);

			// Chromatic dispersion for liquid depth
			vec2 rOffset = warpedUv + vec2(0.0035 * r.x * u_intensity, 0.0);
			vec2 bOffset = warpedUv - vec2(0.0035 * r.y * u_intensity, 0.0);

			vec4 curR = texture2D(u_image, clamp(rOffset, 0.002, 0.998));
			vec4 curG = texture2D(u_image, warpedUv);
			vec4 curB = texture2D(u_image, clamp(bOffset, 0.002, 0.998));
			vec4 curColor = vec4(curR.r, curG.g, curB.b, curG.a);

			vec4 color = curColor;

			if (u_mix < 1.0) {
				vec4 nxtR = texture2D(u_next_image, clamp(rOffset, 0.002, 0.998));
				vec4 nxtG = texture2D(u_next_image, warpedUv);
				vec4 nxtB = texture2D(u_next_image, clamp(bOffset, 0.002, 0.998));
				vec4 nextColor = vec4(nxtR.r, nxtG.g, nxtB.b, nxtG.a);
				color = mix(curColor, nextColor, u_mix);
			}

			// Gentle breathing pulse
			float pulse = sin(t * 0.7) * 0.03 + 1.0;
			gl_FragColor = vec4(color.rgb * pulse, color.a);
		}
	`;

	function createShader(gl: WebGLRenderingContext, type: number, source: string): WebGLShader | null {
		const shader = gl.createShader(type);
		if (!shader) return null;
		gl.shaderSource(shader, source);
		gl.compileShader(shader);
		if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
			console.error('Shader compile error:', gl.getShaderInfoLog(shader));
			gl.deleteShader(shader);
			return null;
		}
		return shader;
	}

	function initGL(canvas: HTMLCanvasElement): boolean {
		try {
			gl = canvas.getContext('webgl', { alpha: false, antialias: true, powerPreference: 'low-power' });
			if (!gl) return false;

			const vs = createShader(gl, gl.VERTEX_SHADER, VS_SOURCE);
			const fs = createShader(gl, gl.FRAGMENT_SHADER, FS_SOURCE);
			if (!vs || !fs) return false;

			program = gl.createProgram();
			if (!program) return false;
			gl.attachShader(program, vs);
			gl.attachShader(program, fs);
			gl.linkProgram(program);

			if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
				console.error('Program link error:', gl.getProgramInfoLog(program));
				return false;
			}

			gl.useProgram(program);

			const posBuf = gl.createBuffer();
			gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
			gl.bufferData(
				gl.ARRAY_BUFFER,
				new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
				gl.STATIC_DRAW
			);

			const posLoc = gl.getAttribLocation(program, 'a_position');
			gl.enableVertexAttribArray(posLoc);
			gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

			return true;
		} catch (e) {
			console.error('Failed to init WebGL:', e);
			return false;
		}
	}

	function loadTexture(gl: WebGLRenderingContext, img: HTMLImageElement): WebGLTexture | null {
		const tex = gl.createTexture();
		if (!tex) return null;
		gl.bindTexture(gl.TEXTURE_2D, tex);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
		return tex;
	}

	function updateImage(newSrc: string) {
		if (!gl || !program || !newSrc) return;
		currentSrc = newSrc;
		const img = new Image();
		img.crossOrigin = 'anonymous';
		img.onload = () => {
			if (!gl || !program || currentSrc !== newSrc) return;
			const newTex = loadTexture(gl, img);
			if (!newTex) return;

			if (!currentTexture) {
				currentTexture = newTex;
				textureMix = 1.0;
			} else {
				nextTexture = newTex;
				textureMix = 0.0;
			}
		};
		img.src = newSrc;
	}

	function render() {
		if (!gl || !program || !canvasEl || !isVisible) return;

		const dpr = Math.min(window.devicePixelRatio || 1, 2);
		const width = Math.floor(canvasEl.clientWidth * dpr);
		const height = Math.floor(canvasEl.clientHeight * dpr);

		if (canvasEl.width !== width || canvasEl.height !== height) {
			canvasEl.width = width;
			canvasEl.height = height;
			gl.viewport(0, 0, width, height);
		}

		if (textureMix < 1.0) {
			textureMix = Math.min(1.0, textureMix + 0.04);
			if (textureMix >= 1.0) {
				if (currentTexture) gl.deleteTexture(currentTexture);
				currentTexture = nextTexture;
				nextTexture = null;
			}
		}

		if (!currentTexture) {
			animId = requestAnimationFrame(render);
			return;
		}

		gl.useProgram(program);

		const elapsed = (performance.now() - startTime - totalPausedDuration) / 1000;
		gl.uniform1f(gl.getUniformLocation(program, 'u_time'), elapsed);
		gl.uniform1f(gl.getUniformLocation(program, 'u_speed'), speed);
		gl.uniform1f(gl.getUniformLocation(program, 'u_intensity'), intensity);
		gl.uniform1f(gl.getUniformLocation(program, 'u_mix'), textureMix);

		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, currentTexture);
		gl.uniform1i(gl.getUniformLocation(program, 'u_image'), 0);

		if (nextTexture) {
			gl.activeTexture(gl.TEXTURE1);
			gl.bindTexture(gl.TEXTURE_2D, nextTexture);
			gl.uniform1i(gl.getUniformLocation(program, 'u_next_image'), 1);
		}

		gl.drawArrays(gl.TRIANGLES, 0, 6);

		if (!playback.paused) {
			animId = requestAnimationFrame(render);
		}
	}

	// Playback pause / resume handling
	$effect(() => {
		const paused = playback.paused;
		if (paused) {
			pausedAt = performance.now();
			cancelAnimationFrame(animId);
		} else {
			if (pausedAt > 0) {
				totalPausedDuration += performance.now() - pausedAt;
				pausedAt = 0;
			}
			cancelAnimationFrame(animId);
			animId = requestAnimationFrame(render);
		}
	});

	// React to source image changes
	$effect(() => {
		if (src && src !== currentSrc) {
			updateImage(src);
		}
	});

	onMount(() => {
		if (!canvasEl) return;

		const ok = initGL(canvasEl);
		if (!ok) {
			webglFailed = true;
			return;
		}

		const obs = new IntersectionObserver((entries) => {
			const entry = entries[0];
			isVisible = entry ? entry.isIntersecting : true;
			if (isVisible && !playback.paused) {
				cancelAnimationFrame(animId);
				animId = requestAnimationFrame(render);
			}
		});
		obs.observe(canvasEl);

		if (src) updateImage(src);
		animId = requestAnimationFrame(render);

		return () => {
			obs.disconnect();
			cancelAnimationFrame(animId);
			if (gl) {
				if (currentTexture) gl.deleteTexture(currentTexture);
				if (nextTexture) gl.deleteTexture(nextTexture);
				if (program) gl.deleteProgram(program);
			}
		};
	});
</script>

{#if webglFailed}
	<img {src} {alt} class="{className} object-cover" />
{:else}
	<canvas
		bind:this={canvasEl}
		aria-label={alt}
		class="{className} block h-full w-full object-cover"
	></canvas>
{/if}
