#version 450

layout(location = 0) in vec3    fragColor;
layout(location = 1) in vec3    fragNormal;
layout(location = 2) in vec3    fragBaseLight;
layout(location = 3) in float   ambientStrength;
layout(location = 4) in vec3    lightPos;
layout(location = 5) in vec3    fragPos;
layout(location = 6) in float   specularStrength;
layout(location = 7) in vec3    viewPos;

layout(location = 0) out vec4   outColor;

void main() {
    // diffusion
    vec3 norm = normalize(fragNormal);
    vec3 lightDir = normalize(lightPos - fragPos);
    vec3 diffuse = max(dot(norm, lightDir), 0.0) * fragBaseLight;
    // specular
    vec3 viewDir = normalize(viewPos - fragPos);
    vec3 halfwayDir = normalize(lightDir + viewDir);
    vec3 reflectDir = reflect(-lightDir, norm);
    float spec = pow(max(dot(norm, halfwayDir), 0.0), 32);
    vec3 specular = specularStrength * spec * fragBaseLight;
    vec3 lightColor = (ambientStrength + diffuse + specular) * fragBaseLight;
    outColor = vec4(fragColor * lightColor, 1.0);
}
