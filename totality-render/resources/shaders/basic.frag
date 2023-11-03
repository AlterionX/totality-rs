#version 450

layout (location = 1) in vec2 uv;
layout (location = 2) in vec3 vert_norm;
layout (location = 3) in vec2 normalized_bc;
layout (location = 4) in vec3 height_adjusted_bc;
layout (location = 5) in vec3 vert_pos;

layout (set = 0, binding = 0, std140) uniform Counts {
    layout(offset = 0) uvec4 count; // instance, light, material, spare
} counts;
layout (set = 0, binding = 1, std140) uniform PerMeshData {
    layout (offset =  0) mat4 orientation;
    layout (offset = 64) vec3 offset;
} per_mesh_data[1024];
layout (set = 0, binding = 2, std140) uniform Lights {
    layout (offset = 0) vec4 color_and_kind;
    layout (offset = 16) vec4 offset;
} lights[1024];
layout (set = 0, binding = 3, std140) uniform Materials {
    layout (offset =  0) vec4 material;
} materials[1024];

layout(set = 1, binding = 0) uniform texture2D tex;
layout(set = 1, binding = 1) uniform sampler samp;

layout (push_constant) uniform Constants {
  layout (offset =  0) mat4 viewport_cam_offori;
  // This is technically not used, but included since our compiler is dumb and requires this to be fully specified.
  layout (offset = 64) bool draw_wireframe;
} push;

layout (location = 0) out vec3 color;

void main() {
    // Wireframe drawing. Takes precedence over all shading.
    if (push.draw_wireframe) {
        // If a barymetric coordinate is "small", we're close to the edge. Otherwise, proceed to normal shading.
        float distance_to_closest_edge = min(min(height_adjusted_bc.x, height_adjusted_bc.y), height_adjusted_bc.z);
        if (distance_to_closest_edge < 0.01) {
            color = vec3(0, 1, 0);
            return;
        }
    }

    vec3 diffuse = vec3(uv, 0.0);
    ivec2 texSize = textureSize(sampler2D(tex, samp), 0);
    if (texSize.x != 1 && texSize.y != 1) {
        diffuse = vec3(texture(sampler2D(tex, samp), uv));
    }

    color = vec3(0, 0, 0);
    // Iterate lights.
    int i;
    for (i = 0; i < counts.count[1]; i++) {
        vec3 light_color = vec3(lights[i].color_and_kind);
        float kind = lights[i].color_and_kind[3];
        if (kind == 1) {
            // Point light
            vec3 to_surface = vert_pos - vec3(lights[i].offset);
            float distance = length(to_surface);
            vec3 effective_direction = normalize(to_surface);
            float direct_component = dot(vert_norm, effective_direction);
            if (direct_component < 0) {
                continue;
            }

            // TODO specular
            vec3 specular_component = vec3(0, 0, 0);

            // diffuse component
            vec3 diffuse_component = direct_component * diffuse * light_color;

            color = color + diffuse_component + specular_component;
        } else if (kind == 2) {
            // Directional light
            vec3 effective_direction = vec3(lights[i].offset);
            float direct_component = dot(vert_norm, effective_direction);
            if (direct_component < 0) {
                continue;
            }

            // diffuse component
            vec3 diffuse_component = direct_component * diffuse * light_color;

            color = color + diffuse_component;
        } else {
            // Unknown light -- ignore!
            color = diffuse;
        }
    }
    if (counts.count[1] == 0) {
        color = diffuse;
    }
}
