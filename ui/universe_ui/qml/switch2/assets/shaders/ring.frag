#version 440
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 size;
    float radius;
    float line;
    float gap;
    float phase;
    float cycles;
    float sharp;
    vec4 c0;
    vec4 c1;
    vec4 c2;
    vec4 c3;
    vec4 inner;
};

// Position along the perimeter in [0,1), uniform speed along the edges, clockwise from the top-left corner
float perimeter(vec2 q) {
    vec2 a = abs(q);
    if (a.x >= a.y) {
        return q.x > 0.0 ? 0.25 + (q.y + 1.0) * 0.125 : 0.75 + (1.0 - q.y) * 0.125;
    }
    return q.y > 0.0 ? 0.5 + (1.0 - q.x) * 0.125 : (q.x + 1.0) * 0.125;
}

vec4 palette(float t) {
    t = fract(t) * 4.0;
    vec4 a = t < 1.0 ? c0 : t < 2.0 ? c1 : t < 3.0 ? c2 : c3;
    vec4 b = t < 1.0 ? c1 : t < 2.0 ? c2 : t < 3.0 ? c3 : c0;
    float k = fract(t);
    k = mix(k, smoothstep(0.0, 1.0, k), sharp);
    return mix(a, b, k);
}

void main() {
    vec2 hs = size * 0.5;
    vec2 p = qt_TexCoord0 * size - hs;
    vec2 d2 = abs(p) - (hs - vec2(radius));
    float d = length(max(d2, 0.0)) + min(max(d2.x, d2.y), 0.0) - radius;
    float aa = 0.8;
    float outer = 1.0 - smoothstep(-aa, aa, d);
    float lineIn = 1.0 - smoothstep(-aa, aa, d + line);
    float gapIn = 1.0 - smoothstep(-aa, aa, d + line + gap);
    float t = perimeter(p / hs) * cycles + phase;
    vec4 band = palette(t) * (outer - lineIn);
    vec4 white = inner * (lineIn - gapIn);
    fragColor = (band + white) * qt_Opacity;
}
