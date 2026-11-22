#version 100
precision mediump float;
uniform sampler2D Texture;
uniform float Reception;
uniform float Clock;
uniform vec2 Pixel;
varying vec2 uv;
float noise(vec2 p){return fract(sin(dot(p,vec2(57.3,19.7)))*673.47);}
void main(){
    vec2 q=uv*2.0-1.0;
    vec2 lens=q*(0.979+0.023*dot(q,q))*0.5+0.5;
    float weak=1.0-Reception;
    float band=step(0.989,noise(vec2(floor(lens.y*91.0),floor(Clock*16.0))));
    lens.x+=band*Pixel.x*weak*1.8;
    vec3 rgb=texture2D(Texture,lens).rgb;
    float luma=dot(rgb,vec3(0.25,0.60,0.15));
    rgb=mix(vec3(luma),rgb,0.68)*vec3(0.965,1.018,1.025);
    rgb=rgb*(0.975+0.025*cos(lens.y/Pixel.y*3.14))+(noise(lens*157.0+fract(Clock))-0.5)*(0.012+weak*0.028);
    float vignette=1.0-0.22*smoothstep(0.36,1.72,dot(q,q));
    gl_FragColor=vec4(rgb*vignette,1.0);
}
