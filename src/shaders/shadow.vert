#version 100
attribute vec3 Position;
uniform mat4 Light;
uniform mat4 Model;
void main(){gl_Position=Light*Model*vec4(Position,1.);}
