// Rspack loader behind `pluginBlurQuery`: reads the import's query string
// (`?sigma=14&sat=1.05&scale=0.5&blur`) and returns the image blurred, as WebP.
import sharp from "sharp";

export const raw = true;

export default function blurLoader(source) {
  const done = this.async();
  const q = new URLSearchParams(this.resourceQuery);
  const sigma = Number(q.get("sigma") || 0) || 8;
  const sat = Number(q.get("sat") || 1);
  const scale = Math.min(1, Number(q.get("scale") || 1)) || 1;
  (async () => {
    let img = sharp(source);
    const { width, height } = await img.metadata();
    // A blur hides detail, so blur a downscaled copy (sigma scales with it);
    // CSS `cover` stretches it back to the plate.
    if (scale < 1 && width && height) img = img.resize(Math.round(width * scale), Math.round(height * scale));
    img = img.blur(Math.max(0.3, sigma * scale));
    if (sat !== 1) img = img.modulate({ saturation: sat });
    return img.webp({ quality: 80 }).toBuffer();
  })().then((buf) => done(null, buf), (err) => done(err));
}
