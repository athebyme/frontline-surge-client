#version 100
precision highp float;
uniform sampler2D Atlas;
uniform sampler2D ShadowMap;
uniform sampler2D LandCover;
uniform vec4 LandRect;
uniform float Fade;
uniform vec3 Eye;
uniform vec3 Sun;
uniform vec4 SkyState;
uniform vec4 LampA;
uniform vec4 LampB;
uniform vec4 LampC;
uniform vec4 LampD;
varying vec3 world;
varying vec3 local;
varying vec3 normal;
varying vec2 uv;
varying vec4 style;
varying vec4 shadow;
varying vec4 fieldEdge;
float hash2(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float noise2(vec2 p){
  vec2 k=floor(p),f=fract(p);f=f*f*(3.-2.*f);
  return mix(mix(hash2(k),hash2(k+vec2(1.,0.)),f.x),mix(hash2(k+vec2(0.,1.)),hash2(k+1.),f.x),f.y);
}
float blocked(vec3 q,vec2 delta,float bias){
  return step(dot(texture2D(ShadowMap,q.xy+delta).rgb,vec3(1.,1./255.,1./65025.))+bias,q.z);
}
float lamp(vec4 p,vec3 n){
  vec3 to=p.xyz-world;
  float d2=dot(to,to);
  float falloff=clamp(1.-d2/max(.001,p.w*p.w),0.,1.);
  return falloff*falloff*(.25+.75*max(0.,dot(to*inversesqrt(max(.0001,d2)),n)));
}
void main(){
  if(Fade<.999 && fract(sin(dot(floor(gl_FragCoord.xy),vec2(127.1,311.7)))*43758.5453)>Fade)discard;
  float tile=floor(style.w+.1);
  float day=SkyState.x,dusk=SkyState.y,moon=SkyState.z;
  vec3 n=normalize(normal),sun=Sun.y>=0.?Sun:-Sun;
  sun=normalize(vec3(sun.x,max(.14,sun.y),sun.z));
  vec3 pigment=style.rgb;
  bool landscape=tile==18.;
  if(landscape){
    vec2 coord=(world.xz-LandRect.xy)*LandRect.zw;
    vec4 cover=texture2D(LandCover,clamp(coord,0.,1.));
    float outside=max(max(-coord.x,coord.x-1.),max(-coord.y,coord.y-1.));
    float mask=1.-smoothstep(.0,.04,outside);
    pigment=mix(vec3(.48,.53,.36),cover.rgb,mask);
    float category=floor(cover.a*255./70.+.5);
    tile=category==1.?2.:category==2.?15.:category>=3.?14.:1.;
  }
  if(tile==22.||tile==23.){
    // Paint is sampled in chassis-local space so the camouflage follows moving vehicles.
    vec2 q=vec2(local.x*.93+local.z*.48,local.y*.84-local.z*.61)*2.1;
    float pattern=noise2(q*.85)*.55+noise2(q*2.25+.73)*.32+noise2(q*4.17+1.9)*.13;
    float low=smoothstep(.485,.590,pattern), high=smoothstep(.60,.699,pattern);
    pigment*=mix(vec3(1.),tile==22.?vec3(.83,.885,.91):vec3(.87,.85,.805),low);
    pigment=mix(pigment,pigment*1.15,high);
    tile=9.;
  }
  if(tile==19.){
    float worn=noise2(world.xz*.9+world.yy*.57)*.72+noise2(uv*3.)*.28;
    // Rain and rising damp expose the brick first along the foot of the facade.
    float damp=(1.-smoothstep(.65,1.9,local.y))*(.50+.50*worn);
    float chip=worn+.10*damp;
    pigment*=mix(vec3(1.),vec3(.68,.71,.66),damp);
    if(chip>.735){
      tile=5.;
      pigment=mix(vec3(.75,.69,.57),vec3(.64,.44,.33),smoothstep(.735,.77,chip));
    }else{
      tile=6.;
      pigment*=1.+.10*smoothstep(.69,.735,chip);
    }
  }else if(tile==20.){
    tile=6.;pigment*=world.y<.84?vec3(.65,.69,.63):vec3(1.);
  }else if(tile==21.){
    tile=9.;pigment*=(.84+.16*abs(cos(uv.x*12.)));
  }else if(tile==24.){
    tile=4.;
    float slat=fract(local.y/.115);
    float profile=mix(.60,1.09,smoothstep(.04,.36,slat))-.12*smoothstep(.81,1.,slat);
    float near=1.-smoothstep(12.,30.,distance(world,Eye));
    pigment*=mix(1.,profile,near*(1.-abs(n.y)));
  }
  vec2 cell=vec2(mod(tile,8.),floor(tile/8.));
  vec3 detail=texture2D(Atlas,(cell+.015625+.96875*fract(uv))/vec2(8.,4.)).rgb;
  float d=distance(world,Eye);
  vec3 base=pigment*mix(detail,vec3(.941),.72*smoothstep(32.,120.,d));
  if(tile==1.||tile==2.||tile==14.||tile==15.){
    float patch=noise2(world.xz*.25)*.68+noise2(world.xz*1.3)*.32;
    base*=.88+.21*patch;
    if(!landscape && fieldEdge.z>0.){
      vec2 margins=fieldEdge.zw-abs(fieldEdge.xy);
      float amount=clamp(min(fieldEdge.z,fieldEdge.w)*.44,.07,1.);
      float cut=(noise2(world.xz*1.7)*.60+noise2(world.xz*4.8)*.25+noise2(world.xz*.7)*.35)*amount;
      if(min(margins.x,margins.y)<cut)discard;
    }
    if(tile==1.)base*=mix(vec3(1.07,1.,.90),vec3(.89,1.025,.93),patch);
  }
  vec3 q=shadow.xyz/shadow.w*.5+.5;
  float incidence=max(0.,dot(n,sun)),vis=1.;
  if(all(greaterThan(q,vec3(.002)))&&all(lessThan(q,vec3(.998)))){
    vec2 t=vec2(.00072);
    float bias=.00072+.0012*(1.-incidence);
    float occ=(blocked(q,t,bias)+blocked(q,-t,bias)+blocked(q,vec2(t.x,-t.y),bias)+blocked(q,vec2(-t.x,t.y),bias))*.25;
    float edge=min(min(q.x,1.-q.x),min(q.y,1.-q.y));
    vis-=occ*smoothstep(.002,.055,edge)*.84;
  }
  vec3 noon=mix(vec3(.49,.53,.55),vec3(.74,.80,.83),n.y*.5+.5);
  vec3 midnight=mix(vec3(.23,.283,.37),vec3(.374,.425,.537),n.y*.5+.5);
  vec3 ambient=mix(midnight,noon,day);
  ambient=mix(ambient,vec3(.493,.441,.479)*(0.82+.18*(n.y*.5+.5)),dusk*.68);
  vec3 direct=mix(vec3(.129,.171,.252),vec3(.535,.476,.352),day);
  direct=mix(direct,vec3(.680,.321,.128),dusk*.81);
  vec3 lit=base*(ambient+direct*incidence*vis);
  if(tile==9.||tile==16.){
    float shine=pow(max(0.,dot(reflect(-sun,n),normalize(Eye-world))),tile==16.?22.:36.);
    lit+=shine*vis*(tile==16.?.40:.18)*mix(vec3(.54,.68,.83),vec3(.99,.93,.82),day);
  }
  if(tile==17.)lit=base*mix(1.34,1.08,day);
  if(day<.99){float local=lamp(LampA,n)+lamp(LampB,n)+lamp(LampC,n)+lamp(LampD,n);
    lit+=base*local*(1.-day)*vec3(1.31,.966,.556);}
  float fog=clamp(1.-exp(-mix(.0000135,.0000105,day)*d*d),0.,.96);
  float toward=dot(normalize(world.xz-Eye.xz+vec2(.0001)),normalize(Sun.xz))*.5+.5;
  vec3 haze=mix(vec3(.175,.221,.271),vec3(.803,.826,.786),day);
  haze=mix(haze,mix(vec3(.505,.388,.401),vec3(.975,.596,.334),toward),dusk*.89);
  gl_FragColor=vec4(mix(lit,haze,fog),1.);
}
