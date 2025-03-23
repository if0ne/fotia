#include "Common.hlsl"
#include "Pbr.hlsl"

SamplerState linear_clamp_s : register(s0);

cbuffer GlobalBuffer : register(b0, space0) {
    Globals g_data;
}

Texture2D diffuse_t : register(t0, space1);
Texture2D normal_t : register(t1, space1);

cbuffer MaterialBuffer : register(b0, space1) {
    Material material_data;
}

cbuffer ObjectTransform : register(b0, space2)
{
    matrix transform;
}

struct VertexInput {
    float3 pos : POSITION;
    float3 normal : NORMAL;
    float2 uv : TEXCOORD;
    float4 tangent: TANGENT;
};

struct PixelInput {
    float4 pos : SV_POSITION;
    float3 pos_w: POSITION;
    float3 normal: NORMAL;
    float3 tangent: TANGENT;
    float3 bitangent : BITANGENT;
    float2 uv : TEXCOORD;
};

PixelInput VSMain(VertexInput input) {
    PixelInput output = (PixelInput) 0;

    float4 world_pos = mul(transform, float4(input.pos, 1.0f));
    output.pos_w = world_pos.xyz;
    output.pos = mul(g_data.proj_view, world_pos);
    output.normal = mul(transform, input.normal);
    output.tangent = mul(transform, input.tangent.xyz);
    output.bitangent = normalize(cross(output.normal, output.tangent));
    output.uv = input.uv;

    return output;
}

struct PixelShaderOutput {
    float4 diffuse : SV_Target0;
    float4 normal : SV_Target1;
    float4 material : SV_Target2;
};

[earlydepthstencil]
PixelShaderOutput PSMain(PixelInput input) {
    PixelShaderOutput output;

    float3 normal_sample = normal_t.Sample(linear_clamp_s, input.uv).rgb * 2.0 - 1.0;

    float3 t = normalize(input.tangent);
    float3 b = normalize(input.bitangent);
    float3 n = normalize(input.normal);
    float3x3 tbn = float3x3(t, b, n);
    float3 normal = normalize(mul(normal_sample, tbn));

    output.diffuse = material_data.diffuse;
    output.normal = pack_normal_to_texture(normal);
    output.material = float4(material_data.fresnel_r0, material_data.roughness, 0.0, input.pos.z);

    return output;
}
