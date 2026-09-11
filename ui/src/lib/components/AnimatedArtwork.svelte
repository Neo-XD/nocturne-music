<script lang="ts">
	import { onMount } from 'svelte';
	import { playback } from '$lib/player.svelte';

	let {
		src,
		alt = '',
		class: className = '',
		style = '',
		intensity = 1.0,
		speed = 1.0
	}: {
		src?: string | null;
		alt?: string;
		class?: string;
		style?: string;
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
		uniform float u_has_image;

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
			float t = u_time * u_speed * 0.7;

			// Dual-octave domain warping (Kawarp style)
			vec2 q = vec2(
				snoise(uv * 2.0 + vec2(t * 0.35, t * 0.25)),
				snoise(uv * 2.0 + vec2(t * 0.25 + 4.3, t * 0.35 + 2.1))
			);

			vec2 r = vec2(
				snoise(uv * 2.8 + 4.0 * q + vec2(t * 0.4 + 1.7, t * 0.45 + 9.2)),
				snoise(uv * 2.8 + 4.0 * q + vec2(t * 0.35 + 8.3, t * 0.4 + 2.8))
			);

			// Fluid offset with boundary clamp - enhanced displacement for visible organic liquid movement
			vec2 warpedUv = uv + r * (0.24 * max(u_intensity, 0.25));
			warpedUv = clamp(warpedUv, 0.002, 0.998);

			// Chromatic dispersion for liquid depth
			vec2 rOffset = clamp(warpedUv + vec2(0.012 * r.x * max(u_intensity, 0.25), 0.004 * r.y), 0.002, 0.998);
			vec2 bOffset = clamp(warpedUv - vec2(0.012 * r.y * max(u_intensity, 0.25), 0.004 * r.x), 0.002, 0.998);

			vec4 color;

			if (u_has_image < 0.5) {
				// Procedural generative liquid mesh when no artwork texture is available
				vec3 c1 = vec3(0.32, 0.55, 0.96); // royal sky/blue
				vec3 c2 = vec3(0.86, 0.32, 0.65); // vibrant magenta/rose
				vec3 c3 = vec3(0.18, 0.78, 0.72); // luminous emerald/teal
				vec3 c4 = vec3(0.10, 0.12, 0.22); // deep midnight space

				float n1 = snoise(warpedUv * 1.8 + vec2(t * 0.25, -t * 0.20)) * 0.5 + 0.5;
				float n2 = snoise(warpedUv * 2.6 - vec2(-t * 0.20, t * 0.24)) * 0.5 + 0.5;
				float n3 = snoise(warpedUv * 3.4 + vec2(t * 0.18, t * 0.14)) * 0.5 + 0.5;

				vec3 grad = mix(c1, c2, n1);
				grad = mix(grad, c3, n2 * 0.75);
				grad = mix(grad, c4, (1.0 - n3) * 0.45);

				color = vec4(grad, 1.0);
			} else {
				vec4 curR = texture2D(u_image, rOffset);
				vec4 curG = texture2D(u_image, warpedUv);
				vec4 curB = texture2D(u_image, bOffset);
				vec4 curColor = vec4(curR.r, curG.g, curB.b, curG.a);

				color = curColor;

				if (u_mix < 1.0) {
					vec4 nxtR = texture2D(u_next_image, rOffset);
					vec4 nxtG = texture2D(u_next_image, warpedUv);
					vec4 nxtB = texture2D(u_next_image, bOffset);
					vec4 nextColor = vec4(nxtR.r, nxtG.g, nxtB.b, nxtG.a);
					color = mix(curColor, nextColor, u_mix);
				}
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
			if (!newTex) {
				webglFailed = true;
				return;
			}

			if (!currentTexture) {
				currentTexture = newTex;
				textureMix = 1.0;
			} else {
				nextTexture = newTex;
				textureMix = 0.0;
			}
		};
		img.onerror = () => {
			// CORS fallback for hosts disallowing anonymous crossOrigin
			const fallbackImg = new Image();
			fallbackImg.onload = () => {
				if (!gl || !program || currentSrc !== newSrc) return;
				try {
					const newTex = loadTexture(gl, fallbackImg);
					if (!newTex) {
						webglFailed = true;
						return;
					}
					if (!currentTexture) {
						currentTexture = newTex;
						textureMix = 1.0;
					} else {
						nextTexture = newTex;
						textureMix = 0.0;
					}
				} catch {
					webglFailed = true;
				}
			};
			fallbackImg.onerror = () => {
				webglFailed = true;
			};
			fallbackImg.src = newSrc;
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

		gl.useProgram(program);

		const elapsed = (performance.now() - startTime - totalPausedDuration) / 1000;
		gl.uniform1f(gl.getUniformLocation(program, 'u_time'), elapsed);
		const effectiveSpeed = playback.paused ? speed * 0.6 : speed;
		gl.uniform1f(gl.getUniformLocation(program, 'u_speed'), effectiveSpeed);
		gl.uniform1f(gl.getUniformLocation(program, 'u_intensity'), intensity);
		gl.uniform1f(gl.getUniformLocation(program, 'u_mix'), textureMix);
		const hasImage = currentTexture ? 1.0 : 0.0;
		gl.uniform1f(gl.getUniformLocation(program, 'u_has_image'), hasImage);

		if (currentTexture) {
			gl.activeTexture(gl.TEXTURE0);
			gl.bindTexture(gl.TEXTURE_2D, currentTexture);
			gl.uniform1i(gl.getUniformLocation(program, 'u_image'), 0);

			if (nextTexture) {
				gl.activeTexture(gl.TEXTURE1);
				gl.bindTexture(gl.TEXTURE_2D, nextTexture);
				gl.uniform1i(gl.getUniformLocation(program, 'u_next_image'), 1);
			}
		}

		gl.drawArrays(gl.TRIANGLES, 0, 6);

		if (isVisible && !document.hidden) {
			animId = requestAnimationFrame(render);
		}
	}

	// Document visibility handling (pause RAF when window is hidden to save battery)
	$effect(() => {
		if (typeof document !== 'undefined' && document.hidden) {
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
		if (src !== currentSrc) {
			if (src) {
				updateImage(src);
			} else {
				currentSrc = '';
				if (currentTexture && gl) {
					gl.deleteTexture(currentTexture);
					currentTexture = null;
				}
				if (nextTexture && gl) {
					gl.deleteTexture(nextTexture);
					nextTexture = null;
				}
			}
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
			if (isVisible && !document.hidden) {
				cancelAnimationFrame(animId);
				animId = requestAnimationFrame(render);
			}
		});
		obs.observe(canvasEl);

		const onVisibility = () => {
			if (document.hidden) {
				cancelAnimationFrame(animId);
			} else if (isVisible) {
				cancelAnimationFrame(animId);
				animId = requestAnimationFrame(render);
			}
		};
		document.addEventListener('visibilitychange', onVisibility);

		if (src) updateImage(src);
		if (!document.hidden) {
			animId = requestAnimationFrame(render);
		}

		return () => {
			document.removeEventListener('visibilitychange', onVisibility);
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
	{#if src}
		<img {src} {alt} class="{className} object-cover" {style} />
	{:else}
		<div class="{className} bg-gradient-to-br from-primary/30 via-accent/20 to-secondary/30" {style}></div>
	{/if}
{:else}
	<canvas
		bind:this={canvasEl}
		aria-label={alt}
		class="{className} block h-full w-full object-cover"
		{style}
	></canvas>
{/if}
