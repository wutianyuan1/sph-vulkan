#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4    model;
    mat4    view;
    mat4    proj;
    vec3    baseLight;
	float   ambientStrength;
    vec3    lightPos;
    float   specularStrength;
    vec3    viewPos;
} ubo;

layout(location = 0) in vec3    inPos;
layout(location = 1) in vec3    inColor;
layout(location = 2) in vec3    inNormal;

layout(location = 0) out vec3   fragColor;
layout(location = 1) out vec3   fragNormal;
layout(location = 2) out vec3   fragBaseLight;
layout(location = 3) out float  ambientStrength;
layout(location = 4) out vec3   lightPos;
layout(location = 5) out vec3   fragPos;
layout(location = 6) out float  specularStrength;
layout(location = 7) out vec3   viewPos;

void main() {
    // position transform
    gl_Position = ubo.proj * ubo.view * ubo.model * vec4(inPos, 1.0);
    fragPos = vec3(ubo.model * vec4(inPos, 1.0));
    // gamma correction
    fragColor = inColor;
    float gamma = 2.2;
    fragColor.rgb = pow(fragColor.rgb, vec3(1.0/gamma));
    // normal transform
    fragNormal = mat3(transpose(inverse(ubo.model))) * inNormal;
    // output values
	fragBaseLight = ubo.baseLight;
	ambientStrength = ubo.ambientStrength;
    lightPos = ubo.lightPos;
    specularStrength = ubo.specularStrength;
	viewPos = ubo.viewPos;
}
