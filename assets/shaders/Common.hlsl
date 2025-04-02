#ifndef COMMON_HLSL
#define COMMON_HLSL

struct Globals {
    matrix view;
    matrix proj;
    matrix proj_view;
    matrix inv_view;
    matrix inv_proj;
    matrix inv_proj_view;

    float3 eye_pos;
    float _pad0;

    float2 screen_dim;
    float2 _pad1;
};

float4 clip_to_world(float4 clip, matrix inv_proj_view) {
    float4 view = mul(inv_proj_view, clip);

    view = view / view.w;

    return view;
}

float4 screen_to_world(float4 screen, float2 screen_dim, matrix inv_proj_view) {
    float2 texCoord = screen.xy / screen_dim;

    float4 clip = float4(float2(texCoord.x, 1.0f - texCoord.y) * 2.0f - 1.0f, screen.z, screen.w);

    return clip_to_world(clip, inv_proj_view);
}

float4 pack_normal_to_texture(float3 normal) {
    normal = normalize(normal);
    float3 packedNormal = normal * 0.5 + 0.5;

    return float4(packedNormal, 1.0);
}

float3 unpack_normal_from_texture(float4 packed_normal) {
    float3 normal = packed_normal.xyz * 2.0 - 1.0;
    return normalize(normal);
}

#endif
