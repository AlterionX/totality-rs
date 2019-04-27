#version 450
layout (location = 0) in vec3 position;
layout (location = 1) in vec2 uv;

layout (push_constant) uniform PushConsts {
  layout (offset = 0) mat4 mvp;
  // float time;
} push;
layout (location = 0) out gl_PerVertex {
    vec4 gl_Position;
};
layout (location = 1) out vec2 frag_uv;

void main() {
    gl_Position = push.mvp * vec4(position, 1.0);
    frag_uv = uv;
}
