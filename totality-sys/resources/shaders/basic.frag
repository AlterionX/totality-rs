#version 450
layout (push_constant) uniform PushConsts {
  vec4 color;
  // float time;
} push;
// layout (location = 1) in vec4 frag_color;
layout (location = 0) out vec4 color;

void main()
{
  // color = clamp(0, 1, push.color);
  // color = clamp(0, 1, push.color + frag_color);
  // color[3] = time;
  color = push.color;
}
