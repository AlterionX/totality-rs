#version 450
layout (location = 0) in vec3 position;
layout (location = 1) in vec2 uv;
layout (location = 2) in vec4 m_0;
layout (location = 3) in vec4 m_1;
layout (location = 4) in vec4 m_2;
layout (location = 5) in vec4 m_3;

layout (push_constant) uniform PushConsts {
  layout (offset = 0) mat4 vp;
  // float time;
} push;
layout (location = 0) out gl_PerVertex {
    vec4 gl_Position;
};
layout (location = 1) out vec2 frag_uv;

void main() {
    gl_Position = push.vp * mat4(m_0, m_1, m_2, m_3) * vec4(position, 1.0);
    frag_uv = uv;
}
