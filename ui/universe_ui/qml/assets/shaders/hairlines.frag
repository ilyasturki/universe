#version 440
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 resolution;
    vec2 center;
    float travel;
    float streak;
    float dp;
    vec4 ink;
};

vec3 hash32(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yzz) * p3.zyx);
}

// One depth layer: a streak per polar sector, recycled in depth as the camera travels, as long as the radius it sweeps.
float layer(float r, float an, float rmax, float N, float seed, float k) {
    float acc = 0.0;
    float s0 = floor(an * N);
    for (int j = -1; j <= 1; j++) {
        float s = mod(s0 + float(j), N);
        vec3 h = hash32(vec2(s, seed));
        float da = an - (s + 0.1 + 0.8 * h.x) / N;
        da -= floor(da + 0.5);
        float perp = abs(da) * 6.2831853 * r;
        float cover = clamp(0.6 * dp + 0.5 - perp, 0.0, 1.0);
        if (cover <= 0.0)
            continue;
        float z = fract(h.y - travel * k);
        float zz = mix(0.04, 1.0, z);
        float R = rmax * (0.04 + 0.22 * h.z);
        float head = R / zz;
        float tail = min(R / (zz + streak * k), head - 1.4 * dp);
        float along = smoothstep(tail - 0.5, tail + 0.5, r) * (1.0 - smoothstep(head - 0.5, head + 0.5, r));
        float g = clamp((r - tail) / max(head - tail, 0.001), 0.0, 1.0);
        float near = (1.0 - smoothstep(0.72, 1.0, z)) * (0.22 + 0.78 * (1.0 - z));
        acc += cover * along * (0.25 + 0.75 * g) * near;
    }
    return acc;
}

void main() {
    vec2 d = qt_TexCoord0 * resolution - center;
    float r = length(d);
    float rmax = length(resolution) * 0.5;
    float an = atan(d.y, d.x) / 6.2831853 + 0.5;
    float a = layer(r, an, rmax, 90.0, 1.0, 1.0)
            + layer(r, an, rmax, 140.0, 2.0, 0.8)
            + layer(r, an, rmax, 200.0, 3.0, 1.2);
    fragColor = ink * min(a, 1.0) * qt_Opacity;
}
