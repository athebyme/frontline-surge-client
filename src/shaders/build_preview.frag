#version 100
precision mediump float;
uniform vec4 Tint;
varying mediump float shade;
void main() {
    gl_FragColor = vec4(Tint.rgb * shade, Tint.a);
}
