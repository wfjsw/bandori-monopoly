/// <reference types="@rsbuild/core/types" />

// Pre-blurred image URL: `?sigma=<n>[&sat=<n>][&scale=<n>]&blur` (the trailing
// `blur` flag is what plugins/pluginBlurQuery.ts keys on and what these
// single-wildcard pattern can match).
declare module "*&blur" {
  const url: string;
  export default url;
}
