// WebGL renderer for Cubism 2 models: textured triangles with premultiplied
// alpha, per-mesh blend modes, stencil clipping masks and a stable draw-order
// sort. Canvas pixels are mapped into the viewport with a fitted orthographic
// projection (y down in canvas space, as in the model files).

import type { MeshFrame } from "./deform";
import type { ModelJson } from "./types";

/** Alpha threshold for a fragment to count as part of a clipping mask. */
const MASK_ALPHA = 0.5;

const VERTEX_SRC = `
attribute vec2 a_pos;
attribute vec2 a_uv;
uniform vec2 u_scale;
uniform vec2 u_offset;
varying vec2 v_uv;
void main() {
  v_uv = a_uv;
  gl_Position = vec4(a_pos * u_scale + u_offset, 0.0, 1.0);
}
`;

const FRAGMENT_SRC = `
precision mediump float;
varying vec2 v_uv;
uniform sampler2D u_tex;
uniform float u_opacity;
uniform vec3 u_mul;
uniform vec3 u_screen;
uniform float u_alpha_test;
void main() {
  vec4 t = texture2D(u_tex, v_uv);
  if (t.a < u_alpha_test) discard;
  // t.rgb is premultiplied. Multiply scales straight and premultiplied color
  // alike; screenColor composites as s + c - s*c on the straight color, i.e.
  // opacity * (s * a + rgb * mul * (1 - s)) on the premultiplied one.
  float a = t.a * u_opacity;
  vec3 rgb = u_opacity * (u_screen * t.a + t.rgb * u_mul * (1.0 - u_screen));
  gl_FragColor = vec4(rgb, a);
}
`;

interface MeshBuffers {
  pos: WebGLBuffer;
  uv: WebGLBuffer;
  idx: WebGLBuffer;
  nIdx: number;
}

function compile(gl: WebGLRenderingContext, type: number, src: string): WebGLShader {
  const sh = gl.createShader(type);
  if (!sh) throw new Error("createShader failed");
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(sh) ?? "unknown";
    gl.deleteShader(sh);
    throw new Error(`shader compile: ${log}`);
  }
  return sh;
}

function link(gl: WebGLRenderingContext, vs: WebGLShader, fs: WebGLShader): WebGLProgram {
  const prog = gl.createProgram();
  if (!prog) throw new Error("createProgram failed");
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.bindAttribLocation(prog, 0, "a_pos");
  gl.bindAttribLocation(prog, 1, "a_uv");
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(prog) ?? "unknown";
    gl.deleteProgram(prog);
    throw new Error(`program link: ${log}`);
  }
  return prog;
}

/**
 * Draws one model's meshes for a set of deformed frames.
 * The canvas box is fitted (letterboxed) into the drawing buffer.
 */
export class Live2DRenderer {
  private gl: WebGLRenderingContext;
  private program: WebGLProgram;
  private uScale: WebGLUniformLocation;
  private uOffset: WebGLUniformLocation;
  private uTex: WebGLUniformLocation;
  private uOpacity: WebGLUniformLocation;
  private uMul: WebGLUniformLocation;
  private uScreen: WebGLUniformLocation;
  private uAlphaTest: WebGLUniformLocation;
  private model: ModelJson | null = null;
  private buffers: MeshBuffers[] = [];
  private textures: WebGLTexture[] = [];
  /** Fitted viewport into the drawing buffer, in device pixels. */
  private view: [number, number, number, number] = [0, 0, 1, 1];
  private disposed = false;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl", {
      alpha: true,
      antialias: true,
      premultipliedAlpha: true,
      stencil: true,
      depth: false,
    });
    if (!gl) throw new Error("WebGL unavailable");
    this.gl = gl;
    const vs = compile(gl, gl.VERTEX_SHADER, VERTEX_SRC);
    const fs = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SRC);
    this.program = link(gl, vs, fs);
    gl.deleteShader(vs);
    gl.deleteShader(fs);

    const loc = (name: string): WebGLUniformLocation => {
      const l = gl.getUniformLocation(this.program, name);
      if (!l) throw new Error(`uniform ${name} missing`);
      return l;
    };
    this.uScale = loc("u_scale");
    this.uOffset = loc("u_offset");
    this.uTex = loc("u_tex");
    this.uOpacity = loc("u_opacity");
    this.uMul = loc("u_mul");
    this.uScreen = loc("u_screen");
    this.uAlphaTest = loc("u_alpha_test");

    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.CULL_FACE);
    gl.enable(gl.BLEND);
    // Textures are straight-alpha lossy WebP (colorbled under transparent
    // pixels, lossless alpha -- see tools/live2d/convert.py). Premultiply on
    // upload so any residual RGB under alpha=0 contributes nothing and the
    // premultiplied blend functions below see consistent colour.
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.useProgram(this.program);
    gl.uniform1i(this.uTex, 0);
    gl.uniform1f(this.uAlphaTest, 0);
  }

  /**
   * Upload one model's UV/index geometry and textures. Positions are written
   * per frame from the deform pipeline; UVs and indices stay fixed.
   */
  setModel(model: ModelJson, textures: readonly TexImageSource[]): void {
    const gl = this.gl;
    this.disposeModel();

    this.model = model;
    this.buffers = model.meshes.map((m) => {
      const pos = gl.createBuffer();
      const uv = gl.createBuffer();
      const idx = gl.createBuffer();
      if (!pos || !uv || !idx) throw new Error("createBuffer failed");
      gl.bindBuffer(gl.ARRAY_BUFFER, pos);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(m.uvs.length), gl.DYNAMIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, uv);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(m.uvs), gl.STATIC_DRAW);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, idx);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(m.indices), gl.STATIC_DRAW);
      return { pos, uv, idx, nIdx: m.indices.length };
    });

    this.textures = textures.map((src) => {
      const tex = gl.createTexture();
      if (!tex) throw new Error("createTexture failed");
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);
      return tex;
    });
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  /** Fit the canvas box into a `css × dpr` drawing buffer (letterboxed). */
  resize(cssWidth: number, cssHeight: number, dpr: number): void {
    const canvas = this.gl.canvas as HTMLCanvasElement;
    const w = Math.max(1, Math.round(cssWidth * dpr));
    const h = Math.max(1, Math.round(cssHeight * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;

    const cw = this.model ? this.model.canvas[0] : w;
    const ch = this.model ? this.model.canvas[1] : h;
    const s = Math.min(w / cw, h / ch);
    const vw = Math.max(1, Math.round(cw * s));
    const vh = Math.max(1, Math.round(ch * s));
    this.view = [Math.round((w - vw) / 2), Math.round((h - vh) / 2), vw, vh];
  }

  /**
   * Draw one frame. `frames[i]` supplies mesh `i`'s composed canvas-space
   * positions and interpolated form scalars (`null` skips the mesh).
   * Draw order is `drawOrder` with a stable tie-break on model order;
   * invisible meshes and zero-opacity meshes are skipped.
   */
  render(frames: readonly (MeshFrame | null)[]): void {
    const gl = this.gl;
    const model = this.model;
    if (!model || this.disposed) return;

    const canvas = gl.canvas as HTMLCanvasElement;
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.disable(gl.SCISSOR_TEST);
    gl.clearColor(0, 0, 0, 0);
    gl.clearStencil(0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.STENCIL_BUFFER_BIT);

    const [vx, vy, vw, vh] = this.view;
    gl.viewport(vx, vy, vw, vh);

    const [w, h] = model.canvas;
    gl.useProgram(this.program);
    gl.uniform2f(this.uScale, 2 / w, -2 / h);
    gl.uniform2f(this.uOffset, -1, 1);

    const order: number[] = [];
    for (let i = 0; i < model.meshes.length; i++) {
      const m = model.meshes[i];
      const f = frames[i];
      if (!m.visible || !f || !f.visible || f.opacity <= 0) continue;
      order.push(i);
    }
    order.sort((a, b) => {
      const fa = frames[a] as MeshFrame;
      const fb = frames[b] as MeshFrame;
      return fa.drawOrder - fb.drawOrder || a - b;
    });

    for (const i of order) {
      this.drawMesh(i, frames);
    }
    gl.disable(gl.STENCIL_TEST);
  }

  private drawMesh(index: number, frames: readonly (MeshFrame | null)[]): void {
    const gl = this.gl;
    const model = this.model as ModelJson;
    const mesh = model.meshes[index];
    const frame = frames[index] as MeshFrame;
    const bufs = this.buffers[index];

    // The mask pass rebinds geometry, so bind this mesh after it.
    if (mesh.clip && mesh.clip.meshes.length > 0) {
      this.beginClip(mesh.clip.meshes, mesh.clip.invert, frames);
    } else {
      gl.disable(gl.STENCIL_TEST);
    }

    gl.bindBuffer(gl.ARRAY_BUFFER, bufs.pos);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, frame.positions);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, bufs.uv);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, bufs.idx);

    const [sr, sg, sb] = frame.screen;
    const [mr, mg, mb] = frame.multiply;
    gl.uniform1f(this.uOpacity, frame.opacity);
    gl.uniform3f(this.uMul, mr, mg, mb);
    gl.uniform3f(this.uScreen, sr, sg, sb);
    gl.uniform1f(this.uAlphaTest, 0);

    const blend = mesh.blend;
    if (blend === 1) {
      gl.blendFunc(gl.DST_COLOR, gl.ONE_MINUS_SRC_ALPHA);
    } else if (blend === 2) {
      gl.blendFunc(gl.ONE, gl.ONE);
    } else if (blend === 3) {
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_COLOR);
    } else {
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    }

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.textures[mesh.tex] ?? null);
    gl.drawElements(gl.TRIANGLES, bufs.nIdx, gl.UNSIGNED_SHORT, 0);
    gl.disable(gl.STENCIL_TEST);
  }

  /**
   * Stencil out the union of the given mask meshes (their deformed coverage),
   * then leave the test set so only the covered (or, inverted, uncovered)
   * region of the next draw shows.
   */
  private beginClip(
    maskIndices: readonly number[],
    invert: boolean,
    frames: readonly (MeshFrame | null)[],
  ): void {
    const gl = this.gl;
    const model = this.model as ModelJson;
    const masks = maskIndices.filter((mi) => model.meshes[mi] && frames[mi]);

    gl.enable(gl.STENCIL_TEST);
    gl.stencilMask(0xff);
    gl.clearStencil(0);
    gl.clear(gl.STENCIL_BUFFER_BIT);
    gl.colorMask(false, false, false, false);

    for (const mi of masks) {
      const mesh = model.meshes[mi];
      const frame = frames[mi] as MeshFrame;
      const bufs = this.buffers[mi];
      gl.bindBuffer(gl.ARRAY_BUFFER, bufs.pos);
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, frame.positions);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      gl.bindBuffer(gl.ARRAY_BUFFER, bufs.uv);
      gl.enableVertexAttribArray(1);
      gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 0, 0);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, bufs.idx);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.textures[mesh.tex] ?? null);
      gl.uniform1f(this.uOpacity, frame.opacity);
      gl.uniform3f(this.uMul, 1, 1, 1);
      gl.uniform3f(this.uScreen, 0, 0, 0);
      gl.uniform1f(this.uAlphaTest, MASK_ALPHA);
      gl.blendFunc(gl.ONE, gl.ZERO);
      // REPLACE 1: overlapping triangles inside one mask stay 1, and the
      // masks accumulate as a union.
      gl.stencilFunc(gl.ALWAYS, 1, 0xff);
      gl.stencilOp(gl.KEEP, gl.KEEP, gl.REPLACE);
      gl.drawElements(gl.TRIANGLES, bufs.nIdx, gl.UNSIGNED_SHORT, 0);
    }

    gl.colorMask(true, true, true, true);
    gl.stencilOp(gl.KEEP, gl.KEEP, gl.KEEP);
    gl.stencilFunc(invert ? gl.NOTEQUAL : gl.EQUAL, 1, 0xff);
    gl.uniform1f(this.uAlphaTest, 0);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  }

  private disposeModel(): void {
    const gl = this.gl;
    for (const b of this.buffers) {
      gl.deleteBuffer(b.pos);
      gl.deleteBuffer(b.uv);
      gl.deleteBuffer(b.idx);
    }
    this.buffers = [];
    for (const t of this.textures) gl.deleteTexture(t);
    this.textures = [];
    this.model = null;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.disposeModel();
    this.gl.deleteProgram(this.program);
  }
}