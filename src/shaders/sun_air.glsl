// Integrate sunlit air up to the visible surface. Both sky and scenery use the
// same light space, so a hole opened in a wall also opens its shaft of light.
vec3 sunlitAir(vec3 color, vec3 direction, float distanceToSurface) {
    float angle = max(0., dot(direction, Sun));
    float daylight = smoothstep(.025, .23, Sun.y);
    if (Graphics.x < .5 || daylight < .001 || distanceToSurface < .4) return color;
    float extent = min(distanceToSurface, 52.);
    vec3 begin = (Light * vec4(Eye, 1.)).xyz * .5 + .5;
    vec3 stepLight = (Light * vec4(direction * extent, 0.)).xyz * .5;
    float count = Graphics.x > 1.5 ? 12. : 6.;
    vec3 delta = stepLight / count;
    vec3 p = begin + .5 * delta;
    float sunlit = 0.;
    // Midpoints stay fixed relative to the ray; no temporal noise or flashing.
    for (int i=0; i<12; ++i) {
        if (float(i) >= count) break;
        vec3 packedDepth = texture2D(ShadowMap, clamp(p.xy, 0., 1.)).rgb;
        float depth = dot(packedDepth, vec3(16711680., 65280., 255.) / 16777215.);
        float border = min(min(p.x, 1.-p.x), min(p.y, 1.-p.y));
        float occlusion = smoothstep(.0002, .0021, p.z-depth)
                        * smoothstep(.003, .06, border) * step(0.,p.z) * step(p.z,1.);
        sunlit += 1.-occlusion;
        p += delta;
    }
    float density = (1.-exp(-extent*.011)) * daylight;
    float scatter = density * (sunlit/count) * (.085 + .38 * pow(angle, 5.));
    vec3 tint = mix(vec3(.84,.85,.80), vec3(1.,.78,.55), SkyState.y);
    return mix(color, tint, scatter);
}
