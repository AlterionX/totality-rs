#version 450

layout (location = 1) in vec2 uv;
layout (location = 2) in vec3 vert_norm;
layout (location = 3) in vec2 normalized_bc;
layout (location = 4) in vec3 height_adjusted_bc;

layout (binding = 0, std140) uniform PerMeshData {
    layout (offset =  0) mat4 orientation;
    layout (offset = 64) vec3 offset;
} per_mesh_data[1024];
layout (binding = 1, std140) uniform Lights {
    layout (offset =  0) mat4 orientation;
    layout (offset = 64) vec3 offset;
} lights[1024];
layout (binding = 2, std140) uniform Materials {
    layout (offset =  0) vec4 material;
} materials[1024];

layout (push_constant) uniform Constants {
  layout (offset =  0) mat4 viewport_cam_offori;
  // This is technically not used, but included since our compiler is dumb and requires this to be fully specified.
  layout (offset = 64) bool draw_wireframe;
} push;
// layout(set = 0, binding = 0) uniform texture2D tex;
// layout(set = 0, binding = 1) uniform sampler samp;

layout (location = 0) out vec4 color;

void main() {
    // Wireframe drawing.
    if (push.draw_wireframe) {
        // If a barymetric coordinate is "small", we're close to the edge. Otherwise, proceed to normal shading.
        float distance_to_closest_edge = min(min(height_adjusted_bc.x, height_adjusted_bc.y), height_adjusted_bc.z);
        if (distance_to_closest_edge < 0.01) {
            color = vec4(0, 1, 0, 1);
            return;
        }
    }

    // if (push.has_tex) {
    //     color = mix(color, texture(sampler2D(tex, samp), uv), 0.5);
    // }

    color = vec4(uv, 0.0, 1.0);
}
