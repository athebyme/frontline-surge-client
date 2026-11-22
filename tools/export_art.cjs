#!/usr/bin/env node
/* Bake editable village definitions into native Rust vertex and index buffers.
 * Usage: node tools/export_art.cjs [output directory] from the project root
 * Geometry is emitted as deduplicated indexed triangles, world-space for authored props,
 * local-space for constructions and wrecks added during play. All numbers little-endian.
 */
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const {geometry,groundColors,patterns,rgb,Collector}=require('./art-source/primitives.cjs');
const models=require('./art-source/village.cjs');
const modern=require('./art-source/modern.cjs');
const landscape=require('./art-source/landscape.cjs');
const assetDir=path.resolve(__dirname,'../assets');
const output=path.resolve(process.argv[2] || assetDir);
fs.mkdirSync(output,{recursive:true});
const map=JSON.parse(fs.readFileSync(path.join(assetDir,'level.json'),'utf8'));
const collector=new Collector();
for(const [key,[color,pattern]] of Object.entries(groundColors))
  collector._pal.set(key,[...rgb(color),patterns.indexOf(pattern)]);
for(const [key,color,pattern] of [...models.materials,...modern.materials])
  collector._pal.set(key,[...rgb(color),patterns.indexOf(pattern)]);
collector._pal.set('landSurface',[1,1,1,18]);
for(const key of ['slateBody','slateArmor'])collector._pal.get(key)[3]=22;
for(const key of ['coyote','clay','dune'])collector._pal.get(key)[3]=23;
const surfaceRect=landscape.writeMask(map,path.join(output,'land-cover.png'));
const shapeCache=new Map(),meshes=[];
function capture(key, callback) {
  collector._collector=new Map();
  callback();
  const data=compile(collector._collector);
  if (data.indices.length) meshes.push({key,...data});
}
function compile(batches) {
  const positions=[],normals=[],colors=[],uv=[],tiles=[],indices=[],seen=new Map();
  for(const [kind,batch] of batches){
    if(!shapeCache.has(kind))shapeCache.set(kind,geometry(kind));
    const mesh=shapeCache.get(kind),a=batch.data;
    for(let i=0;i<batch.used;i+=18){
      const sx=Math.hypot(a[i],a[i+1],a[i+2]),sy=Math.hypot(a[i+4],a[i+5],a[i+6]),sz=Math.hypot(a[i+8],a[i+9],a[i+10]);
      if(Math.min(sx,sy,sz)<1e-5)continue;
      const top=a[i+16],bot=a[i+17],pattern=Math.round(a[i+15]);
      const r=a[i+12],g=a[i+13],b=a[i+14];
      const red=Math.round(r*255),green=Math.round(g*255),blue=Math.round(b*255);
      for(let j=0;j<mesh.length;j+=18){
        const tri=[];
        for(let k=0;k<3;k++){
          const v=j+k*6;
          const ox=mesh[v],oy=mesh[v+1],oz=mesh[v+2],nx=mesh[v+3],ny=mesh[v+4],nz=mesh[v+5];
          const fac=bot+(top-bot)*(oy+.5);
          const local=[ox*fac,oy,oz*fac];
          const pos=[0,1,2].map(q => local[0]*a[i+q]+local[1]*a[i+4+q]+local[2]*a[i+8+q]+a[i+3+q*4]);
          const nY=ny+(Math.abs(ny)<.001?(bot-top)*.5:0);
          const normal=[0,1,2].map(q => nx*a[i+q]/(sx*sx)+nY*a[i+4+q]/(sy*sy)+nz*a[i+8+q]/(sz*sz));
          const nl=Math.hypot(...normal)||1;
          for(let q=0;q<3;q++)normal[q]/=nl;
          let px=(ox+.5)*sx,py=(oy+.5)*sy,pz=(oz+.5)*sz;
          let u=Math.abs(ny)>.65?px:(Math.abs(nx)>Math.abs(nz)?pz:px);
          let vCoord=Math.abs(ny)>.65?pz:py;
          if((pattern===3||pattern===4)&&Math.abs(ny)<.65&&sy>Math.max(sx,sz))[u,vCoord]=[vCoord,u];
          const soil=[1,2,14,15].includes(pattern);
          if(soil&&normal[1]>.8){u=pos[0];vCoord=pos[2];}
          const freq=soil?.56:pattern===8?.56:pattern===5?.74:1.20;
          u*=freq;vCoord*=freq;
          // Horizontal field planes have a fixed up normal. Its three spare channels encode
          // half-extents and a sentinel for softly eroding the borders of paths in the shader.
          if(kind==='plane'){
            normal[0]=sx*.5;normal[1]=-2;normal[2]=sz*.5;
            u=ox*sx;vCoord=oz*sz;
          }
          const key=[...pos,...normal,u,vCoord].map(v=>Math.round(v*100000)).concat(red,green,blue,pattern).join('/');
          let index=seen.get(key);
          if(index===undefined){
            index=positions.length/3;seen.set(key,index);positions.push(...pos);normals.push(...normal);
            colors.push(red,green,blue,255);uv.push(u,vCoord);tiles.push(pattern);
          }
          tri.push(index);
        }
        // Retain counterclockwise front faces and authored smooth/flat normals.
        indices.push(...tri);
      }
    }
  }
  return {positions,normals,colors,uv,tiles,indices};
}
for(let i=0;i<map.props.length;i++){
  const p=map.props[i];
  // Close houses/walls come from Rust's shared destructible grid. Their distant
  // silhouettes are baked below; detailed solid shells would waste GPU memory.
  if(!['house','barn','ruin','wall','barricade'].includes(p.kind))capture('p/'+i,()=>{
    if(modern.objects[p.kind] || p.kind==='pole' || ['tankBattery','shippingContainer'].includes(p.style)){
      collector.group(p.x,0,p.z,p.yaw||0,()=>{
        if(p.style==='tankBattery')modern.tankBattery(collector,p);
        else if(p.style==='shippingContainer')modern.shippingContainer(collector,p);
        else if(p.kind==='pole')modern.streetLamp(collector,p);
        else {modern.objects[p.kind](collector);if(p.kind==='mbt')collector.group(0,1.73,.34,0,()=>modern.objects.mbt_turret(collector));}
      });
    }else if(p.kind==='oilwell'){
      // Pumpjack is authored in local units. Scale it to the actual site footprint and collision height.
      collector.group(p.x,0,p.z,p.yaw||0,()=>{
        const m=collector._groups.at(-1);
        const factors=[p.w/2.3,p.h/2.52,p.d/1.83];
        for(let c=0;c<3;c++)for(let axis=0;axis<3;axis++)m[c*3+axis]*=factors[c];
        models.prop(collector,{...p,x:0,z:0,yaw:0});
      });
    }else models.prop(collector,p);
  });
  if(['house','barn','tree','pine','bush'].includes(p.kind))
    capture('lod/'+i,()=>landscape.lod(collector,p));
}
capture('furniture',()=>{
  modern.telecom(collector,map.objective,models.mast);
});
const bounds=map.bounds, cx=(bounds.maxX+bounds.minX)*.5,cz=(bounds.maxZ+bounds.minZ)*.5;
capture('terrain',()=>collector._patch(cx,-.015,cz,bounds.maxX-bounds.minX+1400,bounds.maxZ-bounds.minZ+1400,'landSurface',0));
capture('backdrop',()=>landscape.backdrop(collector,map));
meshes.push({key:'backdrop',...landscape.hills(map)});
const cell=48;
for(let i=Math.floor(bounds.minX/cell);i<=Math.floor(bounds.maxX/cell);i++)
  for(let j=Math.floor(bounds.minZ/cell);j<=Math.floor(bounds.maxZ/cell);j++)
    capture('flora/'+i+'/'+j,()=>landscape.detailCell(collector,map,i,j,cell));
// Newly built cover and scrap heaps are placed in local space by the simulation.
for(const p of [
  {kind:'sandbag',w:3.15,h:1.02,d:.82,built:true},
  {kind:'logs',w:2,h:.8,d:1.05,resource:'wood'},
  {kind:'wall',w:3.2,d:.8,h:1.2},
  {kind:'refinery',w:3.4,d:3.1,h:3.0},
  {kind:'garage',w:4.4,d:4.2,h:2.8},
  {kind:'workshop',w:3,d:2.2,h:2.4},
]){
  capture('local/'+p.kind,()=>models.prop(collector,{x:0,z:0,yaw:0,...p}));
}
for(const kind of modern.vehicleClasses){
  for(const team of ['red','blue'])capture('local/'+kind+'_'+team,()=>modern.factionVehicle(collector,kind,team));
  capture('local/wreck_'+kind,()=>modern.wreck(collector,kind));
  capture('local/wheel_'+kind,()=>modern.roadwheel(collector,kind));
}
for(const turret of ['mbt_turret','apc_turret'])for(const team of ['red','blue'])capture('local/'+turret+'_'+team,()=>modern.factionVehicle(collector,turret,team));
for(let frame=0;frame<12;frame++)capture('local/belt_'+frame,()=>modern.trackBelt(collector,frame));
for(const key of ['wood','metal','oil','fuel','powder','ammo'])capture('local/drop_'+key,()=>modern.pickup(collector,key));
capture('local/refinery_hardware',()=>modern.refineryHardware(collector));
capture('local/refinery_fan',()=>modern.refineryFan(collector));
capture('local/refinery_gauge',()=>collector.box(0,0,0,.26,.030,.014,'readyLamp'));
for(const mode of ['working','idle','ready'])capture('local/lamp_'+mode,()=>modern.refineryLamp(collector,mode));
// Visible add-ons track the same service/utility and mobility kits as server snapshots.
for(const kind of modern.vehicleClasses){
  const [halfWidth,height,tail]=kind==='patrol4x4'||kind==='scout_buggy'?[.7,.89,-1.19]:kind==='cargo6x6'?[1.04,1.40,-2.29]:[1.17,1.54,-1.84];
  for(const side of [-1,1])for(const kit of ['utility','mobility']){
    capture('local/'+kind+'_'+kit+'_'+(side>0?'2':'1'),()=>{
      const x=side*halfWidth;
      if(kit==='mobility'){
        collector.box(x,height+.11,tail,.32,.27,.53,'darkWood');
        for(const z of [-.17,.17])collector.box(x,height+.12,tail+z,.34,.29,.045,'steel');
        collector.cylinder(x,height+.36,tail,.07,.057,.34,'iron',8,0,1.13);
      }else if(kind==='mbt'){
        collector.box(x,height*.79,-.52,.16,.58,1.62,'truckDark',0,0,side*.09);
        for(const z of [-1.10,-.54,.13])collector.box(x+side*.09,height*.81,z,.034,.115,.10,'paleSteel');
      }else if(kind==='cargo6x6'){
        collector.box(x,height+.33,-.77,.11,.75,2.99,'weatherWood');
        for(const z of [-2.05,-1.11,-.15,.6])collector.box(x+side*.04,height+.34,z,.12,.85,.069,'iron');
      }else{
        collector.box(x,height+.25,tail,.25,.57,.33,'truckDark');
        collector.box(x,height+.58,tail,.12,.062,.18,'iron');
        collector.box(x,height+.25,tail-.177,.043,.45,.019,'mustard',0,0,.35);
      }
    });
  }
}
capture('local/freight',()=>{
  collector.box(0,.25,0,.8,.50,.62,'weatherWood');
  for(const x of [-.27,.27])collector.box(x,.25,0,.072,.53,.64,'iron');
});
for(const [team,mat] of [['blue','flagBlue'],['red','flagRed']]){
  capture('local/flag_'+team,()=>{
    collector.cylinder(0,.40,0,.025,.020,.81,'wood',5);
    collector.box(.17,.69,.018,.34,.21,.035,mat,0,0,.05);
    collector.box(.32,.68,-.029,.15,.18,.026,mat,-.47,0,-.04);
    collector.box(.11,.68,.041,.035,.13,.015,'stencil');
  });
}
// Every immutable GPU page is below the 16-bit index ceiling, including the ground foliage batch.
const pages=[];
for(const m of meshes) {
  let remap=new Map(), vertices=[], indices=[];
  function flush(){
    if (indices.length) pages.push({name:m.key,vertices,indices});
    remap=new Map();vertices=[];indices=[];
  }
  for(let i=0;i<m.indices.length;i+=3){
    if(vertices.length+3>60000)flush();
    for(const old of m.indices.slice(i,i+3)){
      if(!remap.has(old)){
        remap.set(old,vertices.length);
        vertices.push({position:m.positions.slice(old*3,old*3+3),normal:m.normals.slice(old*3,old*3+3),
          uv:m.uv.slice(old*2,old*2+2),style:[...m.colors.slice(old*4,old*4+3),m.tiles[old]]});
      }
      indices.push(remap.get(old));
    }
  }
  flush();
}
const rectBytes=Buffer.alloc(16);surfaceRect.forEach((v,i)=>rectBytes.writeFloatLE(v,i*4));
const segments=[Buffer.from('FRQ2'),rectBytes];
const u32=n=>{const b=Buffer.allocUnsafe(4);b.writeUInt32LE(n);return b;};
segments.push(u32(pages.length));
for(const {name,vertices,indices} of pages){
  const key=Buffer.from(name);
  const vbytes=Buffer.allocUnsafe(vertices.length*36);
  let offset=0;
  for(const v of vertices){
    for(const x of [...v.position,...v.normal,...v.uv]){vbytes.writeFloatLE(x,offset);offset+=4;}
    for(const b of v.style)vbytes.writeUInt8(b,offset++);
  }
  const ibytes=Buffer.allocUnsafe(indices.length*2);
  indices.forEach((v,i)=>ibytes.writeUInt16LE(v,i*2));
  segments.push(u32(key.length),key,u32(vertices.length),u32(indices.length),vbytes,ibytes);
}
const art=Buffer.concat(segments);
const head=Buffer.alloc(8);head.write('FZQ2',0);head.writeUInt32LE(art.length,4);
const packed=Buffer.concat([head,require('node:zlib').deflateSync(art,{level:7})]);
fs.writeFileSync(path.join(output,'province.frmesh'),packed);
console.log('Exported '+pages.length+' u16-indexed native GPU mesh pages ('+packed.length+' compressed bytes).');
