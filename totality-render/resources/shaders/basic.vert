#version 450

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 norm;
layout (location = 2) in vec2 uv;

layout (set = 0, binding = 3, std140) uniform Counts {
    layout(offset = 0) vec4 count; // instance, light, material, spare
} counts;
layout (set = 0, binding = 1, std140) uniform PerMeshData {
    layout (offset =  0) mat4 offset_orientation;
} per_mesh_data[1024];
layout (set = 0, binding = 2, std140) uniform Lights {
    layout (offset = 0) vec4 offset;
    layout (offset = 16) vec4 color;
} lights[1024];
layout (set = 0, binding = 3, std140) uniform Materials {
    layout (offset =  0) vec4 material;
} materials[1024];

layout (push_constant) uniform Constants {
  layout (offset =  0) mat4 viewport_cam_offori;
  // This is technically not used, but included since our compiler is dumb and requires this to be fully specified.
  layout (offset = 64) bool draw_wireframe;
} push;

layout (location = 0) out gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
};
layout (location = 1) out vec2 vert_uv;
layout (location = 2) out vec3 vert_norm;
layout (location = 3) out vec3 vert_pos;

void main() {
    mat4 model_offori = per_mesh_data[gl_InstanceIndex].offset_orientation;
    vec4 world_pos = model_offori * vec4(position, 1);

    gl_Position = push.viewport_cam_offori * world_pos;
    vert_uv = uv;
    vert_norm = norm;
    vert_pos = vec3(world_pos);
}
