#version 450
layout (location = 0) in vec3 position;
// layout (location = 1) in float color;
layout (location = 0) out gl_PerVertex {
    vec4 gl_Position;
};
// layout (location = 1) out vec4 frag_color;

void main() {
    gl_Position = vec4(position, 1.0);
    // frag_color = vec4(0, 0, 0, color);
}
