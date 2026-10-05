// Compiled Cubism 2 `model.json` contract -- see docs/LIVE2D-SCHEMA.md.
// These types mirror the converter output (tools/live2d/convert.py) exactly.

/** Blend mode on a drawable: 0 normal, 1 multiply, 2 additive, 3 screen. */
export type BlendMode = 0 | 1 | 2 | 3;

export interface ModelParam {
  id: string;
  min: number;
  max: number;
  def: number;
  repeat: boolean;
}

/** One parameter axis of a keyform grid; `param` indexes `model.params`. */
export interface GridBind {
  param: number;
  keys: number[];
}

/**
 * A corner of the keyform grid: `[axis, keyIndex]` pairs (sorted by axis).
 * An empty `at` is the base form of a grid with no bindings.
 */
export interface GridAt {
  at: [number, number][];
  form: number;
}

export interface KeyformGrid {
  binds: GridBind[];
  at: GridAt[];
}

export interface ClipJson {
  /** Mesh indices whose unioned/intersected coverage masks this mesh. */
  meshes: number[];
  invert: boolean;
}

export interface MeshJson {
  name: string;
  /** Texture index into `model.textures`. */
  tex: number;
  visible: boolean;
  /** Index into `deformers`, `-1` = root. */
  parent: number;
  blend: BlendMode;
  doubleSided: boolean;
  clip: ClipJson | null;
  /** Flat `x,y,…` texture coordinates, one pair per vertex. */
  uvs: number[];
  /** Triangle list. */
  indices: number[];
  /**
   * One flat `x,y,…` vertex array per keyform, in the parent deformer's
   * local space (cage params for warp parents, pivot offsets for rot
   * parents, canvas pixels for the root).
   */
  forms: number[][];
  /** Per form. */
  opacity: number[];
  /** Per form. */
  drawOrder: number[];
  /** Per form, `[r, g, b, a]`. */
  multiplyColor: number[][];
  /** Per form, `[r, g, b, a]`. */
  screenColor: number[][];
  grid: KeyformGrid;
}

interface DeformerBase {
  name: string;
  /** Index into `deformers`, `-1` = root. */
  parent: number;
  visible: boolean;
  grid: KeyformGrid;
}

/** Free-form deformation cage. */
export interface WarpDeformerJson extends DeformerBase {
  kind: "warp";
  /** Lattice points along the cage's u axis (>= 2). */
  cols: number;
  /** Lattice points along the cage's v axis (>= 2). */
  rows: number;
  /** One flat lattice (`x,y,…`, row-major) per keyform, in the parent's space. */
  forms: number[][];
}

/** Pivot rotation; one row per keyform. */
export interface RotDeformerJson extends DeformerBase {
  kind: "rot";
  /** `[angle, originX, originY, scale, reflectX, reflectY]` per form. */
  rot: number[][];
}

export type DeformerJson = WarpDeformerJson | RotDeformerJson;

export interface ModelJson {
  id: string;
  /** `[width, height]` in canvas pixels. */
  canvas: [number, number];
  /** Pixels per unit; equals the canvas width. */
  ppu: number;
  /** Texture file names relative to the model directory. */
  textures: string[];
  params: ModelParam[];
  meshes: MeshJson[];
  deformers: DeformerJson[];
}