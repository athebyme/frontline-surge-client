#version 100
attribute vec3 Position;
attribute vec3 Normal;
attribute vec2 Texcoord;
attribute vec4 Style;
uniform mat4 ViewProjection;
uniform mat4 Light;
uniform mat4 Model;
varying vec3 world;
varying vec3 local;
varying vec3 normal;
varying vec2 uv;
varying vec4 style;
varying vec4 shadow;
varying vec4 fieldEdge;
void main(){
    vec4 p=Model*vec4(Position,1.);
    local=Position;world=p.xyz;fieldEdge=vec4(0.,0.,-1.,-1.);
    if(Normal.y < -1.5){
        normal=vec3(0.,1.,0.); uv=p.xz*.56; fieldEdge=vec4(Texcoord,Normal.xz);
    }else{
        normal=normalize(mat3(Model)*Normal);uv=Texcoord;
    }
    style=vec4(Style.rgb/255., Style.a);shadow=Light*p;gl_Position=ViewProjection*p;
}
