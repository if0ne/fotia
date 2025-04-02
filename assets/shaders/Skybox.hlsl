#include "Common.hlsl"

SamplerState linear_clamp_s : register(s0);

cbuffer GlobalBuffer : register(b0, space0) {
    Globals g_data;
}

TextureCube skybox_t : register(t0, space1);

struct VertexInput {
    float3 pos : POSITION;
};

struct PixelInput {
    float4 pos : SV_POSITION;
    float3 pos_l : POSITION;
};

PixelInput VSMain(VertexInput input) {
    PixelInput output = (PixelInput) 0;

    float4 world_pos = float4(input.pos, 1.0f);
    world_pos += float4(g_data.eye_pos, 0.0);
    output.pos = mul(g_data.proj_view, world_pos).xyww;
    output.pos_l = input.pos;

    return output;
}

[earlydepthstencil]
float4 PSMain(PixelInput input) : SV_TARGET {
    return skybox_t.Sample(linear_clamp_s, input.pos_l);
}
