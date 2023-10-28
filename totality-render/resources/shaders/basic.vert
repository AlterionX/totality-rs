#version 450

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 norm;
layout (location = 2) in vec2 uv;

layout (binding = 0, std140) uniform PerMeshData {
    layout (offset =  0) mat4 offset_orientation;
} per_mesh_data[1024];
layout (binding = 1, std140) uniform PointLights {
    layout (offset =  0) vec4 position;
} lights[1024];

layout (push_constant) uniform Constants {
  layout (offset =  0) mat4 viewport_cam_offori;
  // This is technically not used, but included since our compiler is dumb and requires this to be fully specified.
  layout (offset = 64) vec4 color;
} push;

layout (location = 0) out gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
};
layout (location = 1) out vec2 frag_uv;
layout (location = 2) out vec3 frag_vert_norm;

void main() {
    mat4 model_offori = per_mesh_data[gl_InstanceIndex].offset_orientation;

    // gl_Position = push.viewport_cam_offori * model_offori * vec4(position, 1.0);
    gl_Position = vec4(position, 1.0);
    frag_uv = uv;
    frag_vert_norm = norm;
}
