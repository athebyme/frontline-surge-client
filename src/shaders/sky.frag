#version 100
precision highp float;
varying vec3 ray;
uniform float Time;
uniform vec4 SkyState;
uniform vec3 Sun;
float h2(vec2 x){return fract(sin(dot(x,vec2(127.1,311.7)))*43758.5453);}
float grad(vec2 p){
  vec2 k=floor(p),f=fract(p);f=f*f*(3.-2.*f);
  return mix(mix(h2(k),h2(k+vec2(1.,0.)),f.x),mix(h2(k+vec2(0.,1.)),h2(k+1.),f.x),f.y);
}
void main(){
  vec3 dir=normalize(ray);
  float h=max(dir.y,0.),day=SkyState.x,dusk=SkyState.y,night=SkyState.z;
  float sun=max(dot(dir,Sun),0.),moon=max(dot(dir,-Sun),0.);
  float toward=dot(normalize(dir.xz+vec2(.0001)),normalize(Sun.xz));
  vec3 horizon=mix(vec3(.175,.221,.271),vec3(.803,.826,.786),day);
  vec3 zenith=mix(vec3(.052,.090,.162),vec3(.34,.54,.653),day);
  horizon=mix(horizon,mix(vec3(.505,.388,.401),vec3(.975,.596,.334),toward*.5+.5),dusk*.89);
  vec3 sky=mix(horizon,zenith,smoothstep(-.024,.82,dir.y));
  sky+=vec3(.28,.16,.058)*pow(sun,12.)*(.22+.78*dusk);
  float az=atan(dir.z,dir.x), el=asin(clamp(dir.y,-1.,1.));
  vec2 stars=vec2(az*103.,el*106.), id=floor(stars);
  float star=step(.971,h2(id))*(1.-smoothstep(.012,.125,length(fract(stars)-vec2(.5))))*(.40+.60*h2(id+7.));
  sky+=vec3(.72,.80,.93)*star*night*smoothstep(.055,.25,h);
  vec2 p=dir.xz/(h+.093)*1.25+vec2(Time*.0035+4.2,1.7);
  float n=grad(p)*.52+grad(p*2.10+2.)*.26+grad(p*4.26)*.14+grad(p*8.83)*.08;
  float veil=smoothstep(.38,.64,n)*smoothstep(.023,.16,h);
  vec3 clouds=mix(mix(vec3(.13,.175,.23),vec3(.24,.294,.335),n),mix(vec3(.636,.680,.690),vec3(.954,.933,.856),smoothstep(.40,.66,n)),day);
  clouds=mix(clouds,mix(vec3(.446,.368,.417),vec3(.851,.585,.467),n),dusk*.85);
  sky=mix(sky,clouds,veil*.84);
  sky+=vec3(.93,.76,.46)*smoothstep(.99965,.99991,sun)*(1.-veil*.77)*smoothstep(-.05,.03,Sun.y);
  sky+=vec3(.75,.83,.86)*smoothstep(.99975,.99992,moon)*(1.-veil*.64)*night;
  sky+=vec3(.07,.096,.132)*pow(moon,63.)*night;
  float ridge=.023+.016*sin(7.*az+1.)+.008*sin(az*15.);
  sky=mix(sky,horizon*vec3(.75,.80,.79),1.-smoothstep(ridge-.0007,ridge+.001,dir.y));
  float mid=.007+.010*sin(az*8.)+.008*sin(az*13.);
  sky=mix(sky,horizon*vec3(.64,.71,.66),1.-smoothstep(mid-.0007,mid+.001,dir.y));
  gl_FragColor=vec4(sky,1.);
}
