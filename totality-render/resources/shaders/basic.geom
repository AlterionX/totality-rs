#version 450

layout (triangles) in;

layout(triangle_strip, max_vertices = 6) out;

layout (location = 0) in gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
} gl_in[];
layout (location = 1) in vec2 vert_uv[];
layout (location = 2) in vec3 vert_norm[];
layout (location = 3) in vec3 vert_pos[];

layout (push_constant) uniform Constants {
  layout (offset =  0) mat4 viewport_cam_offori;
  // This is technically not used, but included since our compiler is dumb and requires this to be fully specified.
  layout (offset = 64) bool draw_wireframe;
} push;

layout (location = 1) out vec2 uv;
layout (location = 2) out vec3 norm;
layout (location = 3) out vec2 normalized_bc;
layout (location = 4) out vec3 height_adjusted_bc;
layout (location = 5) out vec3 pos;

void main() {
    // Emit each vertex, generating a barycentric coord.
    int i;
    for (i = 0; i < gl_in.length(); i++) {
        vec3 pos0 = vert_pos[i];
        vec3 pos1 = vert_pos[(i + 1) % 3];
        vec3 pos2 = vert_pos[(i + 2) % 3];

        vec3 leg_cw = pos1 - pos0;
        vec3 leg_ccw = pos2 - pos0;
        vec3 base = pos2 - pos1;

        // The cross product is the same for every vertex. Can we pull this out?
        vec3 cross_product = cross(leg_cw.xyz, leg_ccw.xyz);
        float cross_magnitude = length(cross_product);

        gl_Position = gl_in[i].gl_Position;
        uv = vert_uv[i];
        pos = vert_pos[i];

        norm = vert_norm[i];
        // near-zero length means that we're dealing with *no* normal. Use the face normal instead.
        if (dot(norm, norm) < 0.1) {
            norm = cross_product / cross_magnitude;
        }

        float height = cross_magnitude / length(base);
        if (isnan(height)) {
            height = 0;
        }
        height_adjusted_bc = vec3(0, 0, 0);
        height_adjusted_bc[i] = height;

        normalized_bc = vec2(0, 0);
        if (i < 2) {
            normalized_bc[i] = 1;
        }

        EmitVertex();
    }

    EndPrimitive();

    // Emit normal line if we're working on a wire frame.
    // if (push.draw_wireframe) {
    //     int i;
    //     for (i = 0; i < gl_in.length(); i++) {
    //         vec4 primary = gl_in[(i + 1) % 3].gl_Position - gl_in[i].gl_Position;
    //         vec4 secondary = gl_in[(i + 2) % 3].gl_Position - gl_in[i].gl_Position;
    //     }
    //     EmitVertex();
    // }
}
