#version 100
precision highp float;
uniform sampler2D Texture;
uniform float Thermal;
uniform float Clock;
varying vec2 uv;
void main(){
    vec3 rgb=texture2D(Texture,uv).rgb;
    float luma=dot(rgb,vec3(.24,.66,.10));
    vec2 p=2.*uv-1.;
    float vignette=1.-.10*smoothstep(.48,1.85,dot(p,p));
    if(Thermal>.5){
        float heat=smoothstep(.035,.97,luma);
        gl_FragColor=vec4((vec3(.035,.043,.052)+vec3(.96,.95,.93)*heat)*vignette,1.);
    }else{
        float phosphor=1.-exp(-4.0*pow(max(luma,0.),.70));
        float grain=fract(sin(dot(floor(gl_FragCoord.xy),vec2(12.9898,78.233))+floor(Clock*20.))*43758.5453)-.5;
        vec3 green=phosphor*vec3(.30,.98,.48)+pow(phosphor,5.)*vec3(.25,.0,.18);
        gl_FragColor=vec4((green+grain*.008)*vignette,1.);
    }
}
