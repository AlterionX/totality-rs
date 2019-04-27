#version 450
layout (location = 1) in vec2 uv;

layout (push_constant) uniform PushConsts {
  layout (offset = 64) vec4 color;
  layout (offset = 80) bool has_tex;
  // float time;
} push;
layout(set = 0, binding = 0) uniform texture2D tex;
layout(set = 0, binding = 1) uniform sampler samp;

layout (location = 0) out vec4 color;

void main() {
    // color = clamp(0, 1, push.color);
    // color = clamp(0, 1, push.color + frag_color);
    // color[3] = time;
    color = push.color;
    if (push.has_tex) {
        color = mix(color, texture(sampler2D(tex, samp), uv), 0.5);
    }
}
