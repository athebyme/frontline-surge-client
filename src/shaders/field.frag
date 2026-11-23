#version 100
#ifdef GL_OES_standard_derivatives
#extension GL_OES_standard_derivatives : enable
#endif
precision highp float;
uniform sampler2D Atlas;
uniform highp sampler2D ShadowMap;
uniform highp sampler2D MovingShadowMap;
uniform mat4 Light;
vec3 sunlitAir(vec3 color, vec3 direction, float distanceToSurface);
uniform float Heat;
uniform float MovingCasters;
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
varying vec4 movingShadow;
varying vec4 fieldEdge;
float hash2(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float noise2(vec2 p){
  vec2 k=floor(p),f=fract(p);f=f*f*(3.-2.*f);
  return mix(mix(hash2(k),hash2(k+vec2(1.,0.)),f.x),mix(hash2(k+vec2(0.,1.)),hash2(k+1.),f.x),f.y);
}
float tap(highp sampler2D map, vec2 uv, vec3 receiver, vec2 gradient){
  // Compare each neighbour against the receiving plane at that texel, not the
  // centre depth. Slanted walls then remain lit instead of acquiring a grid.
  float z=receiver.z+dot(gradient,uv-receiver.xy);
  float depth=dot(texture2D(map,uv).rgb,vec3(16711680.,65280.,255.)/16777215.);
  return step(depth,z);
}
float visibilityLoss(highp sampler2D map, vec4 projected, float incidence, vec2 slope, float radius){
  vec3 q=projected.xyz/projected.w*.5+.5;
  if(any(lessThanEqual(q,vec3(.001)))||any(greaterThanEqual(q,vec3(.999))))return 0.;
  float texel=Graphics.y;
  vec2 gradient=slope*(2.*radius/219.9);
  q.z-=(.026+texel*radius*(1.-incidence)*.85)/219.9;
  vec2 pixel=q.xy/texel;
  float coverage;
  if(Graphics.x<1.5){
    vec2 grid=pixel-.5;
    vec2 base=(floor(grid)+.5)*texel, f=fract(grid);
    coverage=mix(mix(tap(map,base,q,gradient),tap(map,base+vec2(texel,0.),q,gradient),f.x),
                 mix(tap(map,base+vec2(0.,texel),q,gradient),tap(map,base+vec2(texel),q,gradient),f.x),f.y);
  }else{
    vec2 f=fract(pixel)-.5, base=(floor(pixel)+.5)*texel;
    vec2 left=.5*(.5-f)*(.5-f), mid=.75-f*f, right=.5*(.5+f)*(.5+f);
    vec2 dx=vec2(texel,0.), dy=dx.yx;
    coverage=left.y*(left.x*tap(map,base-dx-dy,q,gradient)+mid.x*tap(map,base-dy,q,gradient)+right.x*tap(map,base+dx-dy,q,gradient))
            +mid.y*(left.x*tap(map,base-dx,q,gradient)+mid.x*tap(map,base,q,gradient)+right.x*tap(map,base+dx,q,gradient))
            +right.y*(left.x*tap(map,base-dx+dy,q,gradient)+mid.x*tap(map,base+dy,q,gradient)+right.x*tap(map,base+dx+dy,q,gradient));
  }
  // Broader penumbra away from the blocker, limited in world metres so that
  // thin objects retain a readable contact shadow on both texture resolutions.
  float depth=dot(texture2D(map,q.xy).rgb,vec3(16711680.,65280.,255.)/16777215.);
  float separation=219.9*max(0.,q.z-depth);
  float spread=(.11+min(.49,separation*.065))/(2.*radius);
  vec2 a=vec2(.94,.34)*spread, b=vec2(-a.y,a.x);
  float broad=.25*(tap(map,q.xy+a,q,gradient)+tap(map,q.xy-a,q,gradient)
                 +tap(map,q.xy+b,q,gradient)+tap(map,q.xy-b,q,gradient));
  coverage=mix(coverage,broad,.48);
  float edge=min(min(q.x,1.-q.x),min(q.y,1.-q.y));
  return coverage*smoothstep(.002,.055,edge);
}
float lamp(vec4 p,vec3 n){
  vec3 to=p.xyz-world;
  float d2=dot(to,to);
  float falloff=clamp(1.-d2/max(.001,p.w*p.w),0.,1.);
  return falloff*falloff*(.25+.75*max(0.,dot(to*inversesqrt(max(.0001,d2)),n)));
}
vec4 materialSample(float tile,vec2 within,float footprint){
  // Each footprint has wrapped gutters. Even grazing ground and UV wrap seams
  // can be filtered without pulling in a neighbouring atlas material.
  float slot=tile>=25.?tile-7.:tile;
  vec2 face=vec2(mod(slot,5.),floor(slot/5.));
  vec2 lower=vec2(mod(slot,8.),floor(slot/8.));
  vec2 fine=(face*400.+2.+396.*within)/2048.;
  vec2 mid=(vec2(1200.,1600.)+lower*100.+2.+96.*within)/2048.;
  vec2 coarse=(vec2(1200.,1900.)+lower*25.+1.+23.*within)/2048.;
  if(footprint<1.)return texture2D(Atlas,fine);
  if(footprint<4.)return mix(texture2D(Atlas,fine),texture2D(Atlas,mid),smoothstep(1.,4.,footprint));
  if(footprint<5.)return texture2D(Atlas,mid);
  if(footprint<16.)return mix(texture2D(Atlas,mid),texture2D(Atlas,coarse),smoothstep(5.,16.,footprint));
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
    // Stencilled, angular disruptor painted in local coordinates on the chassis.
    vec2 p=vec2(local.x+.43*local.z,local.y-.56*local.z)*.83;
    vec2 stamp=fract(p+vec2(.29,.52)*floor(p.yx*.61));
    float stripe=step(.30,stamp.x+.42*stamp.y)*step(stamp.x+.42*stamp.y,.70)*step(.17,stamp.y);
    pigment*=mix(vec3(1.),tile==22.?vec3(.74,.84,.88):vec3(.82,.75,.65),stripe);
    tile=9.;
  }
  if(tile==19.){
    // Limewashed walls sit on a continuous mineral plinth with a narrow coping.
    float height=local.y;
    if(height<.58){
      tile=28.;pigment=mix(pigment,vec3(.46,.49,.47),.66);
    }else if(height<.66){
      tile=7.;pigment=mix(pigment,vec3(.96,.885,.70),.58);
    }else tile=6.;
  }else if(tile==20.){
    tile=6.;pigment*=local.y<.72?vec3(.71,.78,.76):vec3(1.);
  }else if(tile==21.){
    tile=27.;
  }else if(tile==24.){
    tile=4.;
    float slat=fract(local.y/.115);
    float profile=mix(.60,1.09,smoothstep(.04,.36,slat))-.12*smoothstep(.81,1.,slat);
    float near=1.-smoothstep(12.,30.,distance(world,Eye));
    pigment*=mix(1.,profile,near*(1.-abs(n.y)));
  }
  float d=distance(world,Eye);
  vec3 viewDir=(Eye-world)/max(.001,d);
  float closeDetail=mix(.32,1.,clamp(Graphics.w,0.,1.))*(1.-smoothstep(8.,68.,d));
  bool fissure=tile==25. || tile==29.;
  float footprint=d*.38/max(.28,abs(dot(n,viewDir)));
#ifdef GL_OES_standard_derivatives
  // Size of the continuous UV before wrapping, expressed in fine texels per pixel.
  footprint=396.*max(length(dFdx(uv)),length(dFdy(uv)));
#endif
  footprint*=mix(1.6,1.,Graphics.w);
  vec4 sampleData=materialSample(tile,fract(uv),fissure?0.:footprint);
  if(fissure && sampleData.a<mix(.27,.72,smoothstep(12.,38.,d)))discard;
  vec3 base=pigment*mix(vec3(.982),sampleData.rgb,.12+.88*closeDetail);
  if(tile==1.||tile==2.||tile==14.||tile==15.||tile==26.){
    if(!landscape && fieldEdge.z>0.){
      vec2 margins=fieldEdge.zw-abs(fieldEdge.xy);
      float amount=clamp(min(fieldEdge.z,fieldEdge.w)*.44,.07,1.);
      float cut=(noise2(world.xz*1.7)*.60+noise2(world.xz*4.8)*.25+noise2(world.xz*.7)*.35)*amount;
      if(min(margins.x,margins.y)<cut)discard;
    }
  }
  if(SkyState.w>1.5){
    // Receivers are the same depth-tested triangles. A hot vehicle changes the
    // signal of its own visible surfaces only, so a wall stays an opaque wall.
    float grain=dot(base,vec3(.25,.6,.15));
    float facet=dot(n,normalize(vec3(.37,.85,.24)));
    float temperature=.155+.14*grain+.036*facet;
    if(Heat>0.){
      float engine=smoothstep(.65,2.4,-local.z)*smoothstep(.2,1.35,local.y);
      float surface=tile==16.?-.22:tile==10.?-.09:tile==17.?-.11:0.;
      float warmth=max(0.,Heat-.3);
      temperature=Heat-.095+surface+.10*facet+.105*(grain-.5)+warmth*.27*engine;
    }
    float haze=1.-exp(-.00135*d);
    gl_FragColor=vec4(vec3(mix(temperature,.18,haze)),1.);
    return;
  }
  float incidence=max(0.,dot(n,sun)),vis=1.;
  if(Graphics.x>.5 && incidence>.012){
    vec3 right=normalize(cross(vec3(0.,1.,0.),sun));
    vec2 slope=vec2(dot(n,right),dot(n,cross(sun,right)))/max(.06,incidence);
    float still=visibilityLoss(ShadowMap,shadow,incidence,slope,69.);
    float moving=0.;
    if(MovingCasters>.5)moving=visibilityLoss(MovingShadowMap,movingShadow,incidence,slope,58.);
    vis-=max(still,moving)*.98;
  }
  float up=n.y*.5+.5;
  // Keep each planar face a clear value. The cool upper hemisphere and a warmer
  // ground bounce separate up-facing planes from walls even outside direct sun.
  vec3 noon=mix(vec3(.385,.395,.376),vec3(.623,.697,.755),up);
  vec3 midnight=mix(vec3(.037,.042,.052),vec3(.086,.106,.139),up);
  vec3 ambient=mix(midnight,noon,day);
  ambient=mix(ambient,vec3(.315,.298,.345)*(.80+.20*up),.64*dusk);
  vec3 direct=mix(vec3(.079,.096,.132),vec3(.73,.688,.564),day);
  direct=mix(direct,vec3(.921,.462,.219),.84*dusk);
  float wrapped=clamp((dot(n,sun)+.14)/1.14,0.,1.);
  vec3 lit=base*(ambient+direct*mix(incidence,wrapped,.45)*vis);
  if(tile==13.)lit+=base*day*.045*max(0.,-dot(n,sun))*vec3(.91,1.,.67);
  if(tile==9.||tile==16.||tile==27.||tile==8.||tile==26.){
    float gloss=mix(tile==16.?.88:tile==9.||tile==27.?.42:.07,sampleData.a,closeDetail);
    float reflection=max(0.,dot(reflect(-sun,n),viewDir));
    float highlight=pow(reflection,mix(10.,64.,gloss))*gloss;
    lit+=highlight*vis*mix(vec3(.094,.115,.15),vec3(.78,.79,.72),day);
    if(tile==16.){
      vec3 reflected=reflect(-viewDir,n);
      float horizon=smoothstep(-.15,.43,reflected.y);
      vec3 glass=mix(vec3(.14,.24,.26),vec3(.54,.77,.85),horizon);
      float rim=pow(1.-max(0.,dot(n,viewDir)),3.);
      lit=mix(lit,glass*(.095+.905*day),(.24+.38*rim)*gloss);
    }
  }
  if(tile==17.)lit=base*mix(1.34,1.08,day);
  if(day<.99){float local=lamp(LampA,n)+lamp(LampB,n)+lamp(LampC,n)+lamp(LampD,n);
    lit+=base*local*(1.-day)*vec3(1.31,.966,.556);}
  float fog=clamp(1.-exp(-mix(.0000095,.0000073,day)*d*d),0.,.94);
  float toward=dot(normalize(world.xz-Eye.xz+vec2(.0001)),normalize(Sun.xz))*.5+.5;
  vec3 haze=mix(vec3(.041,.055,.073),vec3(.737,.836,.862),day);
  haze=mix(haze,mix(vec3(.47,.379,.458),vec3(.999,.611,.370),toward),dusk*.89);
  vec3 color=sunlitAir(mix(lit,haze,fog),-viewDir,d);
  float luminance=dot(color,vec3(.2126,.7152,.0722));
  gl_FragColor=vec4(mix(vec3(luminance),color,Graphics.z),1.);
}
