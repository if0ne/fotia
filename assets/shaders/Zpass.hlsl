#include "Common.hlsl"
#include "Pbr.hlsl"

cbuffer GlobalBuffer : register(b0, space0) {
    Globals g_data;
}

cbuffer ObjectTransform : register(b1, space1)
{
    matrix transform;
}

struct VertexInput {
    float3 pos : POSITION;
};

struct PixelInput {
    float4 pos : SV_POSITION;
};

PixelInput Main(VertexInput input) {
    PixelInput output = (PixelInput) 0;

    float4 world_pos = mul(transform, float4(input.pos, 1.0f));
    output.pos = mul(g_data.proj_view, world_pos);

    return output;
}
