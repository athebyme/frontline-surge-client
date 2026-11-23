#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D Texture;
uniform float Reception;
uniform float Clock;
uniform vec2 Pixel;
varying vec2 uv;
// Keep coordinates within the mediump range even at the far corner of a 960px feed.
float noise(vec2 p){return fract(sin(dot(mod(p,251.0),vec2(57.3,19.7)))*673.47);}
void main(){
    vec2 q=uv*2.0-1.0;
    vec2 lens=q*(0.979+0.023*dot(q,q))*0.5+0.5;
    float weak=1.0-clamp(Reception,0.0,1.0);
    float tick=mod(floor(Clock*14.0),251.0);
    float band=noise(vec2(floor(lens.y*32.0),tick));
    float torn=smoothstep(1.0-0.27*weak,1.0,band);
    lens.x+=Pixel.x*torn*weak*12.0*(noise(vec2(tick,9.0))-0.5);
    vec3 rgb=texture2D(Texture,lens).rgb;
    float luma=dot(rgb,vec3(0.25,0.60,0.15));
    rgb=mix(vec3(luma),rgb,mix(0.83,0.35,weak))*vec3(0.965,1.018,1.025);
    float grain=noise(floor(lens/Pixel)+vec2(mod(tick,127.0),0.0))-0.5;
    rgb=rgb*(0.987+0.013*cos(lens.y/Pixel.y*3.14)) + grain*(0.007+weak*weak*0.24);
    rgb=mix(rgb,vec3(luma*0.65+0.11),torn*weak*0.35);
    gl_FragColor=vec4(rgb*(1.0-0.18*smoothstep(0.36,1.72,dot(q,q))),1.0);
}
