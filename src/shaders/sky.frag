#version 100
precision highp float;
varying vec3 ray;
uniform float Time;
uniform vec4 SkyState;
uniform vec3 Sun;
uniform vec3 Eye;
uniform mat4 Light;
uniform vec4 Graphics;
uniform highp sampler2D ShadowMap;
vec3 sunlitAir(vec3 color, vec3 direction, float distanceToSurface);

float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float cloudNoise(vec2 p){
  vec2 k=floor(p), f=fract(p); f=f*f*(3.-2.*f);
  return mix(mix(hash(k),hash(k+vec2(1.,0.)),f.x),
             mix(hash(k+vec2(0.,1.)),hash(k+vec2(1.)),f.x),f.y);
}
float cloudDensity(vec2 p){
  mat2 turn=mat2(.8,-.6,.6,.8);
  return .54*cloudNoise(p)+.30*cloudNoise(turn*p*2.08+3.)
         +.16*cloudNoise(p*4.21-7.);
}
void main(){
  vec3 dir=normalize(ray);
  float day=SkyState.x,dusk=SkyState.y,night=SkyState.z;
  float sun=max(dot(dir,Sun),0.),moon=max(dot(dir,-Sun),0.);
  float toward=dot(normalize(dir.xz+vec2(.0001)),normalize(Sun.xz))*.5+.5;
  vec3 horizon=mix(vec3(.041,.055,.073),vec3(.737,.836,.862),day);
  horizon=mix(horizon,mix(vec3(.47,.379,.458),vec3(.999,.611,.370),toward),dusk*.89);
  vec3 zenith=mix(vec3(.011,.018,.036),vec3(.25,.42,.56),day);
  vec3 sky=mix(horizon,zenith,smoothstep(-.035,.85,dir.y));
  float duskGlow=pow(sun,10.)*(.11+.53*dusk);
  sky+=vec3(.31,.16,.058)*duskGlow;
  float az=atan(dir.z,dir.x);
  vec2 stars=vec2(az*108.,asin(clamp(dir.y,-1.,1.))*104.);
  float star=step(.981,hash(floor(stars)))*(1.-smoothstep(.015,.13,length(fract(stars)-.5)));
  sky+=vec3(.58,.66,.78)*star*night*smoothstep(.035,.22,dir.y);
  // A continuous cloud layer projected onto the sky. Sample toward the sun
  // for the lit rim; the same bank shades its thicker centre.
  vec2 p=dir.xz*2.8/max(.12,dir.y+.12)+vec2(Time*.0025,.6);
  float mass=cloudDensity(p);
  float cover=smoothstep(.47,.67,mass)*smoothstep(.015,.14,dir.y);
  float lightSide=cloudDensity(p+Sun.xz*.18);
  float underside=clamp(.32+2.*(mass-.52)+2.3*(lightSide-mass),0.,1.);
  vec3 cloudLit=mix(vec3(.062,.077,.097),vec3(.994,.972,.902),day);
  vec3 cloudShade=mix(vec3(.032,.044,.060),vec3(.728,.809,.846),day);
  cloudLit=mix(cloudLit,vec3(.936,.662,.469),dusk*.80);
  cloudShade=mix(cloudShade,vec3(.484,.424,.490),dusk*.79);
  sky=mix(sky,mix(cloudLit,cloudShade,underside*.79),cover*.91);
  sky+=vec3(.96,.81,.52)*smoothstep(.99962,.99988,sun)*(1.-cover*.91)*smoothstep(-.04,.04,Sun.y);
  sky+=vec3(.70,.78,.85)*smoothstep(.99970,.99990,moon)*(1.-cover*.84)*night;
  sky+=vec3(.025,.032,.044)*pow(moon,64.)*night;
  if(SkyState.w>1.5){
    float cool=mix(.14,.072,smoothstep(-.05,.78,dir.y));
    sky=vec3(cool+cover*.012);
  }else sky=sunlitAir(sky,dir,52.);
  gl_FragColor=vec4(sky,1.);
}
