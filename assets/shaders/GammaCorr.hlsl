#include "FullscreenVS.hlsl"

Texture2D accum_t : register(t0);
SamplerState linear_clamp_s : register(s0);

float4 Main(FullscreenVertex input) : SV_TARGET
{
    float4 color = accum_t.Sample(linear_clamp_s, input.uv);

    color = pow(color, 1 / 2.2);

    return float4(color.xyz, 1.0);
}
