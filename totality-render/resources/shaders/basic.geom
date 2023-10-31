#version 450

layout (triangles) in;

layout(triangle_strip, max_vertices = 3) out;

layout (location = 0) in gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
} gl_in[];
layout (location = 1) in vec2 vert_uv[];
layout (location = 2) in vec3 vert_vert_norm[];

layout (push_constant) uniform Constants {
  layout (offset =  0) mat4 viewport_cam_offori;
  // This is technically not used, but included since our compiler is dumb and requires this to be fully specified.
  layout (offset = 64) vec4 color;
} push;


layout (location = 1) out vec2 uv;
layout (location = 2) out vec3 vert_norm;
layout (location = 3) out vec2 bc;

void main() {
    // Emit each vertex, generating a barycentric coord.
    int i;
    for (i = 0; i < gl_in.length(); i++) {
        gl_Position = gl_in[i].gl_Position;
        vert_norm = vert_vert_norm[i];
        uv = vert_uv[i];

        vec2 working_bc = vec2(0, 0);
        if (i < 2) {
            working_bc[i] = 1;
        }
        bc = working_bc;

        EmitVertex();
    }

    EndPrimitive();
}
