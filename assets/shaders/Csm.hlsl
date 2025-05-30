cbuffer CsmMatrix : register(b0, space0)
{
    matrix proj_view;
}

cbuffer ObjectTransform : register(b0, space1)
{
    matrix transform;
}

struct VertexInput
{
    float3 pos : POSITION;
};

struct PixelInput
{
    float4 pos : SV_POSITION;
};

PixelInput VSMain(VertexInput input)
{
    PixelInput output;
    output.pos = mul(proj_view, mul(transform, float4(input.pos, 1.0f)));

    return output;
}

float PSMain(PixelInput input) : SV_TARGET
{
    return input.pos.z;
}
