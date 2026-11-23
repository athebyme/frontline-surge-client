#version 100
precision highp float;
uniform sampler2D Atlas;
uniform sampler2D ShadowMap;
uniform sampler2D LandCover;
uniform vec4 LandRect;
// Shadow mode, reciprocal shadow-map size, saturation, close material detail.
uniform vec4 Graphics;
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
vec4 materialSample(vec2 cell,vec2 within,float footprint){
  // Each footprint has wrapped gutters. Even grazing ground and UV wrap seams
  // can be filtered without pulling in a neighbouring atlas material.
  vec2 atlasSize=vec2(2048.,1024.);
  vec2 fine=(cell*256.+2.+252.*within)/atlasSize;
  vec2 mid=(vec2(1536.,768.)+cell*64.+2.+60.*within)/atlasSize;
  vec2 coarse=(vec2(0.,768.)+cell*16.+1.+14.*within)/atlasSize;
  if(footprint<4.)return texture2D(Atlas,fine);
  if(footprint<10.)return mix(texture2D(Atlas,fine),texture2D(Atlas,mid),smoothstep(4.,10.,footprint));
  if(footprint<18.)return texture2D(Atlas,mid);
  if(footprint<34.)return mix(texture2D(Atlas,mid),texture2D(Atlas,coarse),smoothstep(18.,34.,footprint));
  return texture2D(Atlas,coarse);
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
    tile=category==1.?2.:category==2.?15.:category==3.?14.:category>=4.?26.:1.;
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
    vec2 facade=vec2(abs(n.x)>.54?local.z:local.x,local.y);
    float worn=noise2(facade*vec2(1.6,2.2))*.65+noise2(facade*7.3)*.24+noise2(facade*17.4)*.11;
    // Scuffs and delamination concentrate at the plinth; narrow rain streaks run vertically.
    float damp=(1.-smoothstep(.18,1.65,local.y))*(.48+.52*worn);
    float runoff=noise2(facade*vec2(5.1,.39));
    float chip=worn+.19*damp;
    pigment*=mix(vec3(1.),vec3(.68,.71,.66),damp);
    pigment*=1.-.095*smoothstep(.57,.85,runoff);
    if(chip>.758){
      tile=5.;
      pigment=mix(vec3(.72,.66,.55),vec3(.65,.425,.295),smoothstep(.758,.794,chip));
    }else{
      tile=6.;
      pigment*=1.+.055*smoothstep(.72,.758,chip);
    }
  }else if(tile==20.){
    tile=6.;pigment*=world.y<.84?vec3(.65,.69,.63):vec3(1.);
  }else if(tile==21.){
    tile=27.;
  }else if(tile==24.){
    tile=4.;
    float slat=fract(local.y/.115);
    float profile=mix(.60,1.09,smoothstep(.04,.36,slat))-.12*smoothstep(.81,1.,slat);
    float near=1.-smoothstep(12.,30.,distance(world,Eye));
    pigment*=mix(1.,profile,near*(1.-abs(n.y)));
  }
  vec2 cell=vec2(mod(tile,8.),floor(tile/8.));
  float d=distance(world,Eye);
  vec3 viewDir=(Eye-world)/max(.001,d);
  float closeDetail=mix(.32,1.,clamp(Graphics.w,0.,1.))*(1.-smoothstep(8.,68.,d));
  bool fissure=tile==25. || tile==29.;
  float footprint=d*mix(1.8,1.,Graphics.w)/max(.28,abs(dot(n,viewDir)));
  vec4 sampleData=materialSample(cell,fract(uv),fissure?0.:footprint);
  if(fissure && sampleData.a<mix(.27,.72,smoothstep(12.,38.,d)))discard;
  vec3 base=pigment*mix(vec3(.94),sampleData.rgb,.055+.945*closeDetail);
  if(tile==1.||tile==2.||tile==14.||tile==15.||tile==26.){
    float patch=noise2(world.xz*.19)*.69+noise2(world.xz*.65)*.31;
    base*=.90+.20*patch;
    if(!landscape && fieldEdge.z>0.){
      vec2 margins=fieldEdge.zw-abs(fieldEdge.xy);
      float amount=clamp(min(fieldEdge.z,fieldEdge.w)*.44,.07,1.);
      float cut=(noise2(world.xz*1.7)*.60+noise2(world.xz*4.8)*.25+noise2(world.xz*.7)*.35)*amount;
      if(min(margins.x,margins.y)<cut)discard;
    }
    if(tile==1.)base*=mix(vec3(1.12,1.015,.84),vec3(.85,1.07,.93),patch);
  }
  vec3 q=shadow.xyz/shadow.w*.5+.5;
  float incidence=max(0.,dot(n,sun)),vis=1.;
  if(Graphics.x>.5 && all(greaterThan(q,vec3(.002))) && all(lessThan(q,vec3(.998)))){
    float bias=.00035+.00094*(1.-incidence);
    float occ;
    if(Graphics.x<1.5){
      occ=blocked(q,vec2(0.),bias);
    }else{
      // A weighted 3x3 footprint. Resolution and kernel scale change together.
      vec2 t=vec2(Graphics.y*1.23,0.);
      float center=blocked(q,vec2(0.),bias)*4.;
      float axes=blocked(q,t,bias)+blocked(q,-t,bias)+blocked(q,t.yx,bias)+blocked(q,-t.yx,bias);
      float corners=blocked(q,t.xx,bias)+blocked(q,-t.xx,bias)+blocked(q,vec2(t.x,-t.x),bias)+blocked(q,vec2(-t.x,t.x),bias);
      occ=(center+2.*axes+corners)/16.;
    }
    float edge=min(min(q.x,1.-q.x),min(q.y,1.-q.y));
    vis-=occ*smoothstep(.002,.055,edge)*.98;
  }
  float up=n.y*.5+.5;
  // Cool sky fill keeps shadowed interiors readable. Warm sun stays directional.
  vec3 noon=mix(vec3(.39,.40,.37),vec3(.66,.746,.79),up);
  vec3 midnight=mix(vec3(.20,.243,.305),vec3(.344,.410,.522),up);
  vec3 ambient=mix(midnight,noon,day);
  ambient=mix(ambient,vec3(.451,.405,.473)*(.80+.20*up),dusk*.72);
  vec3 direct=mix(vec3(.162,.203,.313),vec3(.67,.565,.397),day);
  direct=mix(direct,vec3(.847,.395,.150),dusk*.89);
  vec3 lit=base*(ambient+direct*incidence*vis);
  if(tile==9.||tile==16.||tile==27.||tile==8.||tile==26.){
    float gloss=mix(tile==16.?.78:tile==9.||tile==27.?.4:.09,sampleData.a,closeDetail);
    float highlight=pow(max(0.,dot(reflect(-sun,n),viewDir)),mix(12.,60.,gloss));
    lit+=highlight*gloss*vis*mix(vec3(.33,.48,.72),vec3(.80,.75,.59),day);
    if(tile==16.){
      float rim=pow(1.-max(0.,dot(n,viewDir)),3.);
      lit=mix(lit,vec3(.42,.62,.74)*(.32+.68*day),(.10+.39*rim)*gloss);
    }
  }
  if(tile==17.)lit=base*mix(1.34,1.08,day);
  if(day<.99){float local=lamp(LampA,n)+lamp(LampB,n)+lamp(LampC,n)+lamp(LampD,n);
    lit+=base*local*(1.-day)*vec3(1.31,.966,.556);}
  float fog=clamp(1.-exp(-mix(.0000135,.0000105,day)*d*d),0.,.96);
  float toward=dot(normalize(world.xz-Eye.xz+vec2(.0001)),normalize(Sun.xz))*.5+.5;
  vec3 haze=mix(vec3(.175,.221,.271),vec3(.79,.846,.839),day);
  haze=mix(haze,mix(vec3(.50,.39,.474),vec3(.994,.600,.354),toward),dusk*.89);
  vec3 color=mix(lit,haze,fog);
  float luminance=dot(color,vec3(.2126,.7152,.0722));
  gl_FragColor=vec4(mix(vec3(luminance),color,Graphics.z),1.);
}
