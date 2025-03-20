#ifndef FULLSCREEN_VS
#define FULLSCREEN_VS

struct FullscreenVertex {
    float4 pos : SV_POSITION;
    float2 uv : TexCoord;
};

FullscreenVertex Main(uint vertex_id : SV_VertexID)
{
    FullscreenVertex result;
    result.uv = float2(vertex_id & 2, (vertex_id << 1) & 2);
    result.pos = float4(result.uv * float2(2.0f, -2.0f) + float2(-1.0f, 1.0f), 0.0f, 1.0f);
    return result;
}

#endif
