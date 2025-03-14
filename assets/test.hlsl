cbuffer ObjectTransform : register(b0)
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
    output.pos = world_pos;

    return output;
}
