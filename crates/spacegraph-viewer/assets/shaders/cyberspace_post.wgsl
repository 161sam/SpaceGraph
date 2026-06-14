// Cyberspace post-process: scanlines + vignette + chromatic aberration + grain.
// Self-contained (no #import) so `naga` can validate it headlessly; the vertex
// stage is Bevy's built-in fullscreen triangle, whose output matches this struct.

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PostFx {
    scanline: f32,
    vignette: f32,
    aberration: f32,
    grain: f32,
    time: f32,
    anomaly_intensity: f32,
    alert_count: u32,
    _pad: f32,
    // Each: xy = screen UV of the alert, z = per-alert intensity, w = 0.
    alerts: array<vec4<f32>, 16>,
};
@group(0) @binding(2) var<uniform> settings: PostFx;

// Cheap hash for film grain.
fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, vec3<f32>(p3.y, p3.z, p3.x) + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let center = vec2<f32>(0.5, 0.5);
    let dir = uv - center;

    // Chromatic aberration: offset the colour channels radially.
    let amt = settings.aberration * 0.012;
    let r = textureSample(screen_texture, texture_sampler, uv - dir * amt).r;
    let g = textureSample(screen_texture, texture_sampler, uv).g;
    let b = textureSample(screen_texture, texture_sampler, uv + dir * amt).b;
    var color = vec3<f32>(r, g, b);

    // Scanlines: subtle horizontal darkening.
    let scan = 1.0 - settings.scanline * (0.5 + 0.5 * sin(uv.y * 800.0));
    color = color * scan;

    // Vignette: radial falloff toward the edges.
    let vig = clamp(1.0 - settings.vignette * dot(dir, dir) * 2.0, 0.0, 1.0);
    color = color * vig;

    // Film grain: time-seeded noise.
    let n = hash12(uv * 1024.0 + vec2<f32>(settings.time, settings.time * 1.7));
    color = color + (n - 0.5) * settings.grain;

    // Anomaly focus (D0/ADR-0012 §3): localize a ripple/desaturation pull toward
    // the most severe/recent alerts so the eye is drawn to *where* it is wrong.
    if (settings.anomaly_intensity > 0.0) {
        var focus = 0.0;
        let count = min(settings.alert_count, 16u);
        for (var i = 0u; i < count; i = i + 1u) {
            let a = settings.alerts[i];
            let d = distance(uv, a.xy);
            let f = clamp(1.0 - d / 0.16, 0.0, 1.0);
            focus = max(focus, f * f * a.z);
        }
        focus = focus * settings.anomaly_intensity;
        // Desaturate toward a danger-red wash and pulse the brightness.
        let lum = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        let wash = vec3<f32>(lum, lum * 0.55, lum * 0.55);
        let pulse = 1.0 + 0.30 * focus * sin(settings.time * 5.0);
        color = mix(color, wash, focus * 0.6) * pulse;
    }

    return vec4<f32>(color, 1.0);
}
