#version 100
attribute vec3 Position;
uniform mat4 Light;
uniform mat4 Model;
varying highp float casterDepth;
void main(){
  gl_Position=Light*Model*vec4(Position,1.);
  casterDepth=gl_Position.z/gl_Position.w*.5+.5;
}
