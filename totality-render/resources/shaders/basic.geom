#version 450

layout (triangles) in;

layout(triangle_strip, max_vertices = 6) out;

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
  layout (offset = 64) bool draw_wireframe;
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
        // near-zero length means that we're dealing with *no* value. Use the face normal instead.
        if (dot(vert_norm, vert_norm) < 0.1) {
            vec4 primary = gl_in[(i + 1) % 3].gl_Position - gl_in[i].gl_Position;
            vec4 secondary = gl_in[(i + 2) % 3].gl_Position - gl_in[i].gl_Position;
            vert_norm = normalize(cross(primary.xyz, secondary.xyz));
        }
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
