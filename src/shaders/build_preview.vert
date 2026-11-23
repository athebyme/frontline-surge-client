#version 100
attribute vec3 Position;
attribute vec3 Normal;
uniform mat4 ViewProjection;
uniform mat4 Model;
varying mediump float shade;
void main() {
    vec3 n = normalize(mat3(Model) * Normal);
    shade = 0.77 + 0.23 * dot(n, normalize(vec3(0.4, 0.8, 0.35)));
    gl_Position = ViewProjection * Model * vec4(Position, 1.0);
}
