'use strict';
// Land cover is baked in metres: the native runtime samples one continuous surface.
// A single road/field texture avoids coplanar decals flickering along distant lanes.
const fs=require('node:fs');
const zlib=require('node:zlib');
const {groundColors,rgb,hash}=require('./primitives.cjs');
const colors=Object.fromEntries(Object.entries(groundColors).map(([k,[h]])=>[k,rgb(h).map(v=>v*255)]));
const clamp=(v,a,b)=>Math.max(a,Math.min(b,v));
const cover={
  meadow:0,grass:0,orchard:0,crop:2,gravel:3,earth:1,dirt:1,road:1,asphalt:4,
  mud:1,forest:0,pasture:0,wheat:2,stubble:2
};
function noise(x,y,seed){
  const i=Math.floor(x),j=Math.floor(y),u=x-i,v=y-j;
  const a=u*u*(3-2*u),b=v*v*(3-2*v);
  return (hash(i,j,seed)*(1-a)+hash(i+1,j,seed)*a)*(1-b)+(hash(i,j+1,seed)*(1-a)+hash(i+1,j+1,seed)*a)*b;
}
function zone(map){
  const b=map.bounds, margin=15;
  return [b.minX-margin,b.minZ-margin,b.maxX-b.minX+2*margin,b.maxZ-b.minZ+2*margin];
}
function writePng(file,width,height,bytes) {
  const polynomial=0xedb88320;
  const crcs=Array.from({length:256},(_,n)=>{
    for(let i=0;i<8;i++)n=(n&1)?(polynomial^(n>>>1)):(n>>>1);
    return n>>>0;
  });
  const parts=[Buffer.from([137,80,78,71,13,10,26,10])];
  function chunk(name,data){
    const tag=Buffer.from(name);
    let crc=0xffffffff;
    for(const b of tag)crc=crcs[(crc^b)&255]^(crc>>>8);
    for(const b of data)crc=crcs[(crc^b)&255]^(crc>>>8);
    const n=Buffer.alloc(4),sum=Buffer.alloc(4);n.writeUInt32BE(data.length);sum.writeUInt32BE((crc^0xffffffff)>>>0);
    parts.push(n,tag,data,sum);
  }
  const hd=Buffer.alloc(13);hd.writeUInt32BE(width);hd.writeUInt32BE(height,4);hd[8]=8;hd[9]=6;chunk('IHDR',hd);
  const stride=width*4;
  const filtered=Buffer.alloc((stride+1)*height);
  for(let j=0;j<height;j++){
    const dest=j*(stride+1);filtered[dest]=1;
    for(let i=0;i<stride;i++)filtered[dest+1+i]=(bytes[j*stride+i]-(i>=4?bytes[j*stride+i-4]:0))&255;
  }
  chunk('IDAT',zlib.deflateSync(filtered,{level:8}));chunk('IEND',Buffer.alloc(0));
  fs.writeFileSync(file,Buffer.concat(parts));
}
function writeMask(map,file){
  const zoneRect=zone(map),[left,front,w,d]=zoneRect;
  const width=Math.max(256,Math.min(2048,Math.ceil(w*3.25/16)*16));
  const height=Math.max(256,Math.min(2048,Math.ceil(d*3.25/16)*16));
  const pixels=Buffer.alloc(width*height*4);
  const base=colors.meadow,grass=colors.grass,stubble=colors.stubble;
  for(let j=0;j<height;j++)for(let i=0;i<width;i++){
    const x=left+(i+.5)*w/width,z=front+(j+.5)*d/height,k=(j*width+i)*4;
    // Broad pasture swaths are painted in metres; individual blades belong to the atlas.
    const parcels=noise(x*.014,z*.014,321);
    const gold=clamp((parcels-.41)*.86,0,.36);
    const swath=Math.floor((x*.88+z*.47)/6.4);
    const cut=(swath&1)===0?1.026:.974;
    for(let c=0;c<3;c++)pixels[k+c]=Math.round((base[c]*(1-gold)+stubble[c]*gold+(grass[c]-base[c])*(1-parcels)*.21)*cut);
  }
  function paint(p,isRoad){
    const paved=isRoad && (p.w>=8 || (p.w>=5.8 && Math.abs(p.x)<3 && Math.abs(p.z)<50));
    const kind=isRoad?(paved?'asphalt':'road'):p.kind||'grass';
    const color=paved?[89,103,109]:(colors[kind]||colors.grass);
    const ca=Math.cos(p.yaw||0),sa=Math.sin(p.yaw||0);
    const ex=Math.abs(ca)*p.w*.5+Math.abs(sa)*p.d*.5+1;
    const ez=Math.abs(sa)*p.w*.5+Math.abs(ca)*p.d*.5+1;
    const lowerX=clamp(Math.floor((p.x-ex-left)*width/w),0,width),upperX=clamp(Math.ceil((p.x+ex-left)*width/w),0,width);
    const lowerZ=clamp(Math.floor((p.z-ez-front)*height/d),0,height),upperZ=clamp(Math.ceil((p.z+ez-front)*height/d),0,height);
    for(let j=lowerZ;j<upperZ;j++)for(let i=lowerX;i<upperX;i++){
      const x=left+(i+.5)*w/width,z=front+(j+.5)*d/height,k=(j*width+i)*4;
      const dx=x-p.x,dz=z-p.z,u=dx*ca+dz*sa,v=-dx*sa+dz*ca;
      const edge=Math.min(p.w*.5-Math.abs(u),p.d*.5-Math.abs(v));
      if(edge<-.6)continue;
      const rough=paved?.17:noise(x*.71,z*.71,7)*.4;
      const alpha=clamp((edge-rough+.30)*2.15,0,1);if(!alpha)continue;
      let tint=1,mark=0;
      if(isRoad){
        const rut=Math.abs(Math.abs(u+.08*Math.sin(v*.34))-Math.min(1.25,p.w*.25));
        tint=1-(paved?.035:.14)*(1-clamp((rut-.07)*2.5,0,1));
        const run=v+p.d*.5,section=Math.floor(run/22);
        // Cold resurfaced rectangles follow the carriageway and leave the painted line visible.
        const side=hash(section,p.x,13)>.5?1:-1;
        const patchU=(u*side)-p.w*.22,patchV=run%22-4;
        if(paved && hash(section,p.z,54)>.56 && Math.abs(patchU)<.86 && Math.abs(patchV)<1.75
            && Math.abs(patchU)+.45*Math.abs(patchV)<1.48)tint*=.88;
        if(paved && Math.abs(v)<p.d*.5-3.6){
          const dash=(v+p.d*.5)%9<4.1 && Math.abs(u)<.12;
          const edgeStripe=Math.abs(Math.abs(u)-(p.w*.5-.55))<.085;
          mark=(dash||edgeStripe)?.81:0;
        }
      }else if(kind==='crop'||kind==='earth'){
        tint=(.96+.08*noise(x*.12,z*.12,113))*(.94+.07*Math.pow(Math.abs(Math.cos(u*1.7)),3));
      }else tint=.98+.035*noise(x*.048,z*.048,113);
      for(let c=0;c<3;c++)pixels[k+c]=Math.round(pixels[k+c]*(1-alpha)+(color[c]*tint*(1-mark)+[225,218,183][c]*mark)*alpha);
      if(alpha>.5)pixels[k+3]=Math.min(252,(cover[kind]||0)*70);
    }
  }
  for(const f of map.fields||[])paint(f,false);
  for(const r of map.roads||[])paint(r,true);
  writePng(file,width,height,pixels);
  return zoneRect;
}

function within(x,z,p,pad=0){
  const dx=x-p.x,dz=z-p.z,ca=Math.cos(p.yaw||0),sa=Math.sin(p.yaw||0);
  return Math.abs(dx*ca+dz*sa)<p.w*.5+pad && Math.abs(-dx*sa+dz*ca)<p.d*.5+pad;
}
function detailCell(r,map,cx,cz,cell) {
  const b=map.bounds,roads=map.roads||[],fields=map.fields||[];
  const x0=cx*cell,z0=cz*cell;
  // Each chunk has ~90 low-poly clusters. Grass/rubble has no collision.
  const props=map.props.filter(p=>Math.abs(p.x-x0-cell*.5)<cell*.5+24&&Math.abs(p.z-z0-cell*.5)<cell*.5+24);
  const rs=roads.filter(p=>Math.abs(p.x-x0-cell*.5)<=cell*.5+(p.w+p.d)*.5+2&&Math.abs(p.z-z0-cell*.5)<=cell*.5+(p.w+p.d)*.5+2);
  const fs=fields.filter(p=>Math.abs(p.x-x0-cell*.5)<=cell*.5+(p.w+p.d)*.5&&Math.abs(p.z-z0-cell*.5)<=cell*.5+(p.w+p.d)*.5);
  const seed=cx*197+cz*793;
  for(let j=0;j<67;j++){
    const x=x0+cell*hash(seed,j,895),z=z0+cell*hash(seed,j,857);
    if(x<b.minX||x>b.maxX||z<b.minZ||z>b.maxZ||rs.some(p=>within(x,z,p,.48))||props.some(p=>within(x,z,p,.22)))continue;
    const field=fs.find(p=>within(x,z,p));
    if(field?.kind==='gravel')continue;
    const v=hash(seed,j,747),h=.14+.22*v;
    if(j%13===0&&field?.kind!=='crop'){
      const s=.08+.14*v;
      r._add('c5',x,.018,z,s,.036,s*.8,'stone',j,0,0,.45,1);continue;
    }
    for(let k=0;k<3;k++){
      const dx=hash(j,k,seed)*1.08-.54,dz=hash(k,j,seed-723)*1.1-.55;
      const plant=field?.kind==='crop'?'stubble':v>.78?'dryGrass':'wildGrass';
      r._add('tuft',x+dx,h*.5,z+dz,h*1.32,h,h*1.5,plant,k*.34+j,0,0);
    }
  }
}
function backdrop(r,map){
  const b=map.bounds, mx=(b.minX+b.maxX)*.5,mz=(b.minZ+b.maxZ)*.5;
  const rx=(b.maxX-b.minX)*.5,rz=(b.maxZ-b.minZ)*.5;
  // Trees behind the edges give each horizon depth without putting art-only obstacles on routes.
  for(let i=0;i<250;i++){
    const side=i%4, t=hash(i,1,34)*2-1;
    const extension=20+hash(i,2,71)*52;
    const x=mx+(side===0?-rx-extension:side===2?rx+extension:t*(rx+29));
    const z=mz+(side===1?-rz-extension:side===3?rz+extension:t*(rz+29));
    const h=5.3+hash(i,3,74)*6.4,w=h*.48;
    r.cylinder(x,h*.25,z,.18,.11,h*.51,'distantTrunk',5);
    if(i%3)r.cylinder(x,h*.6,z,w*.49,0,h*.81,'distantTree',6,i);
    else r._add('crownFar',x,h*.65,z,w,h*.7,w*.94,'distantTree',i,0,0);
  }
}
function lod(r,p){
  const {x,z,w,d,h,yaw=0}=p;
  r.group(x,0,z,yaw,()=>{
    if(p.kind==='pine'||p.kind==='tree'){
      r.cylinder(0,h*.25,0,h*.049,h*.028,h*.49,'trunk',5);
      if(p.kind==='pine'){
        r.cylinder(0,h*.52,0,w*.52,.03,h*.62,'needles',6);
        r.cylinder(0,h*.75,0,w*.36,.01,h*.50,'spruce',6);
      }else{
        r._add('crownFar',0,h*.68,0,w*.91,h*.62,d*.91,'leaf',0,0,0);
        r._add('crownFar',w*.16,h*.78,d*.06,w*.655,h*.40,d*.65,'leafSun',.5,0,0);
      }
    }else if(p.kind==='bush'){
      r._add('crownFar',0,h*.52,0,w,h,d,'hedge',0,0,0);
    }else{
      const isBarn=p.kind==='barn',base=h*(isBarn?.63:.71);
      r.box(0,base*.5,0,w,base,d,isBarn?'barnBoard':'lime');
      const slate=isBarn||p.variant%3===0;
      r.gable(0,base+(h-base)*.5,0,w+.34,h-base,d+.38,slate?'slate':'terracotta');
      r.box(0,base-.075,0,w+.16,.15,d+.16,isBarn?'darkWood':'creamPaint');
      if(!isBarn)r.box(0,.27,0,w+.045,.54,d+.045,'concrete');
      for(const side of [-1,1]){
        const front=side*(d*.5+.03);
        if(isBarn){
          r.box(0,base*.40,front,w*.55,base*.76,.08,'darkWood');
          r.box(0,base*.79,front+side*.05,w*.60,.13,.13,'weatherWood');
          r.box(0,base*.41,front+side*.05,.075,base*.73,.06,'weatherWood');
        }else for(const q of [-1,1]){
          const window=q*w*.26;
          r.box(window,base*.65,front,w*.12,base*.25,.06,'quietGlass');
          r.box(window,base*.52,front+side*.03,w*.16,.13,.12,'limestone');
          r.box(window+w*.08,base*.65,front,w*.04,base*.26,.08,'shutters');
        }
      }
      if(!isBarn)r.box(-w*.26,h*.83,-d*.24,.55,h*.32,.6,'brickwork');
    }
  });
}
// A low-poly collar of rolling hills begins outside all gameplay coordinates.
// It frames long views from vehicles without introducing invisible collision slopes.
function hills(map) {
  const b=map.bounds,cx=(b.minX+b.maxX)*.5,cz=(b.minZ+b.maxZ)*.5;
  const rx=(b.maxX-b.minX)*.5,rz=(b.maxZ-b.minZ)*.5;
  const positions=[],uv=[],indices=[];
  const rings=[8,28,53,84,122,173,238,315],heights=[-.45,.9,5.6,13.7,22.8,14,19,5],num=112;
  for(let j=0;j<rings.length;j++){
    for(let i=0;i<num;i++){
      const a=i/num*Math.PI*2,dx=Math.cos(a),dz=Math.sin(a);
      const offset=rings[j]*(.93+.12*Math.sin(a*8.+.41*j));
      const radius=Math.min((rx+offset)/Math.max(.000001,Math.abs(dx)),(rz+offset)/Math.max(.000001,Math.abs(dz)));
      const x=cx+radius*dx,z=cz+radius*dz;
      const ripple=.67+.55*noise(x*.009,z*.009,31)+.34*Math.sin(a*5.+j*.37);
      const y=j===0?-.45:heights[j]*ripple;
      positions.push(x,y,z);uv.push(x*.26,z*.26);
      if(j===rings.length-1)continue;
      const k=j*num+i,m=j*num+(i+1)%num;
      indices.push(k,m,k+num,m,m+num,k+num);
    }
  }
  // Duplicate these distant vertices once at bake time. A shared normal would
  // smooth away the ridge planes and make the backdrop look like a gradient.
  const result={positions:[],normals:[],uv:[],colors:[],tiles:[],indices:[]};
  for(let a=0;a<indices.length;a+=3){
    const tri=indices.slice(a,a+3);
    const pts=tri.map(i=>positions.slice(i*3,i*3+3));
    const u=pts[1].map((n,k)=>n-pts[0][k]),v=pts[2].map((n,k)=>n-pts[0][k]);
    const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
    const len=(Math.hypot(...n)||1)*(n[1]<0?-1:1),flat=n.map(x=>x/len);
    const tint=.96+.055*hash(Math.floor(a/6),41,3);
    for(let j=0;j<3;j++){
      const i=tri[j];result.indices.push(result.tiles.length);
      result.positions.push(...pts[j]);result.normals.push(...flat);result.uv.push(uv[i*2],uv[i*2+1]);
      result.tiles.push(1);result.colors.push(Math.round(108*tint),Math.round(143*tint),Math.round(103*tint),255);
    }
  }
  return result;
}
module.exports={writeMask,zone,detailCell,backdrop,lod,hills};
