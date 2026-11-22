#version 100
precision highp float;
void main(){
  vec3 depthBytes=fract(gl_FragCoord.z*vec3(1.,255.,65025.));
  depthBytes.xy-=depthBytes.yz/255.;
  gl_FragColor=vec4(depthBytes,1.);
}
