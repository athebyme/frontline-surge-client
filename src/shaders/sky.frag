#version 100
precision highp float;
varying vec3 ray;
uniform float Time;
uniform vec4 SkyState;
uniform vec3 Sun;

float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float octagon(vec2 p,vec2 radius){
  vec2 q=abs(p)/radius;
  return max(max(q.x,q.y),.72*(q.x+q.y))-1.;
}
float cloud(vec2 p,out float underside){
  vec2 id=floor(p);
  id.x=mod(id.x,8.);
  vec2 q=fract(p)-.5;
  q.y+=(hash(id+3.)-.5)*.19;
  q.x+=(hash(id+8.)-.5)*.09;
  float hull=octagon(q-vec2(.005,-.015),vec2(.28,.070));
  hull=min(hull,octagon(q-vec2(-.08,.037),vec2(.120,.089)));
  hull=min(hull,octagon(q-vec2(.083,.016),vec2(.112,.075)));
  underside=1.-smoothstep(-.035,.028,q.y+.015*q.x);
  return (1.-smoothstep(-.045,.075,hull))*step(.35,hash(id));
}
void main(){
  vec3 dir=normalize(ray);
  float day=SkyState.x,dusk=SkyState.y,night=SkyState.z;
  float sun=max(dot(dir,Sun),0.),moon=max(dot(dir,-Sun),0.);
  float toward=dot(normalize(dir.xz+vec2(.0001)),normalize(Sun.xz))*.5+.5;
  vec3 horizon=mix(vec3(.041,.055,.073),vec3(.737,.836,.862),day);
  horizon=mix(horizon,mix(vec3(.47,.379,.458),vec3(.999,.611,.370),toward),dusk*.89);
  vec3 zenith=mix(vec3(.011,.018,.036),vec3(.145,.459,.725),day);
  vec3 sky=mix(horizon,zenith,smoothstep(-.035,.85,dir.y));
  float duskGlow=pow(sun,10.)*(.11+.53*dusk);
  sky+=vec3(.31,.16,.058)*duskGlow;
  float az=atan(dir.z,dir.x);
  vec2 stars=vec2(az*108.,asin(clamp(dir.y,-1.,1.))*104.);
  float star=step(.981,hash(floor(stars)))*(1.-smoothstep(.015,.13,length(fract(stars)-.5)));
  sky+=vec3(.58,.66,.78)*star*night*smoothstep(.035,.22,dir.y);
  // A drifting layer of cut-paper cumulus banks. Flat undersides share the sky fill.
  vec2 p=vec2(az/6.2831853*8.,dir.y*3.8+.07)+vec2(Time*.00024,0.);
  float underside;
  float cover=cloud(p,underside)*smoothstep(.045,.16,dir.y);
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
  }
  gl_FragColor=vec4(sky,1.);
}
