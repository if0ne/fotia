#ifndef PBR_HLSL
#define PBR_HLSL

struct Material {
    float4 diffuse;
    float fresnel_r0;
    float roughness;
};

struct MaterialEntry {
    float fresnel_r0;
    float roughness;
};

float3 schlick_fresnel(float3 r0, float3 normal, float3 light_vec) {
    float cos = saturate(dot(normal, light_vec));
    float f0 = 1.0f - cos;
    float3 reflect_percent = r0 + (1.0f - r0)*(f0*f0*f0*f0*f0);

    return reflect_percent;
}

float3 blinn_phong(float4 diffuse_albedo, MaterialEntry material, float3 light_strength, float3 light_vec, float3 normal, float3 to_eye)
{
    const float m = (1.0 - material.roughness) * 256.0f;
    float3 half_vec = normalize(to_eye + light_vec);

    float roughness_factor = (m + 8.0f)*pow(max(dot(half_vec, normal), 0.0f), m) / 8.0f;
    float3 fresnel_factor = schlick_fresnel(material.fresnel_r0, half_vec, light_vec);

    float3 spec_albedo = fresnel_factor * roughness_factor;

    spec_albedo = spec_albedo / (spec_albedo + 1.0f);

    return (diffuse_albedo.rgb + spec_albedo) * light_strength;
}

#endif
