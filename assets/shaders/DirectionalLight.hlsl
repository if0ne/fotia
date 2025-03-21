#include "Common.hlsl"
#include "ShadowCommon.hlsl"
#include "Light.hlsl"
#include "Pbr.hlsl"
#include "FullscreenVS.hlsl"

SamplerComparisonState comp_shadow_s : register(s0, space0);

cbuffer GlobalBuffer : register(b0, space0) {
    Globals g_data;
}

Texture2D diffuse_t : register(t0, space1);
Texture2D normal_t : register(t1, space1);
Texture2D material_t : register(t2, space1);


cbuffer LightBuffer : register(b0, space1)
{
    DirectionalLight dir_light;
    AmbientLight ambient_light;
}

Texture2D csm_t : register(t0, space2);

cbuffer CsmBuffer : register(b0, space2) {
    CsmData csm_data;
}

float3 compute_dir_light(float4 diffuse, float3 normal, float3 to_eye, float4 material) {
    float3 lightVec = -dir_light.direction;

    float ndotl = max(dot(lightVec, normal), 0.0f);
    float3 light_strength = dir_light.strength * ndotl;

    MaterialEntry mat;
    mat.fresnel_r0 = material.x;
    mat.roughness = material.y;

    return blinn_phong(diffuse, mat, light_strength, lightVec, normal, to_eye);
}

float4 Main(FullscreenVertex input) : SV_Target {
    int2 tex_coord = input.pos.xy;

    float4 diffuse = diffuse_t.Load(int3(tex_coord, 0));
    float3 normal = unpack_normal_from_texture(normal_t.Load(int3(tex_coord, 0)));
    float4 material = material_t.Load(int3(tex_coord, 0));

    float depth = material.w;
    float4 world_pos = screen_to_world(float4(tex_coord, depth, 1.0f), g_data.screen_dim, g_data.inv_proj_view);

    float fragment_dist = mul(g_data.view, world_pos).z;
    uint cascade_idx = 0;

     for (uint i = 0; i < 4 - 1; ++i)
    {
        if (fragment_dist > csm_data.split_distances[i])
        {
            cascade_idx = i + 1;
        }
    }

    float shadow_factor = sample_csm_atlas(csm_t, comp_shadow_s, mul(csm_data.proj_view[cascade_idx], world_pos), cascade_idx);

    float3 to_eye = normalize(g_data.eye_pos - world_pos.xyz);
    float4 ambient = ambient_light.color * diffuse;

    float4 dir_light_color = float4(shadow_factor * compute_dir_light(diffuse, normal, to_eye, material), 1.0);
	float4 lit_color = ambient + dir_light_color;

	lit_color.a = diffuse.a;
    return lit_color;
}
