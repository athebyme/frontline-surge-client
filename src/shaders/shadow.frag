#version 100
precision highp float;
varying highp float casterDepth;
void main(){
  // Use the same high-precision clip coordinate as the receiver. The ES2
  // built-in fragment coordinate may be mediump even in a highp shader.
  float bits=min(16777215.,floor(clamp(casterDepth,0.,1.)*16777215.+.5));
  vec3 bytes=vec3(floor(bits/65536.),mod(floor(bits/256.),256.),mod(bits,256.));
  gl_FragColor=vec4(bytes/255.,1.);
}
