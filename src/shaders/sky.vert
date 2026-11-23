#version 100
attribute vec2 Position;
uniform mat4 InverseView;
varying vec3 ray;
void main(){
  vec4 farP=InverseView*vec4(Position,1.,1.);
  vec4 nearP=InverseView*vec4(Position,-1.,1.);
  ray=farP.xyz/farP.w-nearP.xyz/nearP.w;
  gl_Position=vec4(Position,.99999,1.);
}
