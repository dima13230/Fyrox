use std::ffi::CString;
use crate::{
    core::{
        color::Color,
        math::{
            vec3::Vec3,
            mat4::Mat4
        }
    },
    renderer::{
        gpu_program::UniformLocation,
        gl,
        geometry_buffer::{
            GeometryBuffer,
            GeometryBufferKind,
            AttributeDefinition,
            AttributeKind
        },
        error::RendererError,
        gpu_program::GpuProgram
    },
    scene::{
        SceneContainer,
        node::Node
    },
};
use crate::renderer::RenderPassStatistics;
use crate::renderer::geometry_buffer::ElementKind;

#[repr(C)]
struct Vertex {
    position: Vec3,
    color: u32
}

pub struct DebugRenderer {
    geometry: GeometryBuffer<Vertex>,
    lines: Vec<Line>,
    vertices: Vec<Vertex>,
    line_indices: Vec<[u32; 2]>,
    shader: DebugShader
}

pub struct DebugShader {
    program: GpuProgram,
    wvp_matrix: UniformLocation,
}

impl DebugShader {
    fn new() -> Result<Self, RendererError> {
        let fragment_source = CString::new(include_str!("shaders/debug_fs.glsl"))?;
        let vertex_source = CString::new(include_str!("shaders/debug_vs.glsl"))?;
        let mut program = GpuProgram::from_source("DebugShader", &vertex_source, &fragment_source)?;
        Ok(Self {
            wvp_matrix: program.get_uniform_location("worldViewProjection")?,
            program
        })
    }

    fn bind(&self) {
        self.program.bind()
    }

    pub fn set_wvp_matrix(&self, mat: &Mat4) {
        self.program.set_mat4(self.wvp_matrix, mat)
    }
}

pub struct Line {
    pub begin: Vec3,
    pub end: Vec3,
    pub color: Color
}

impl DebugRenderer {
    pub(in crate) fn new() -> Result<Self, RendererError> {
        let geometry = GeometryBuffer::new(GeometryBufferKind::DynamicDraw, ElementKind::Line);

        geometry.describe_attributes(vec![
            AttributeDefinition { kind: AttributeKind::Float3, normalized: false },
            AttributeDefinition { kind: AttributeKind::UnsignedByte4, normalized: true },
        ])?;

        Ok(Self {
            geometry,
            shader: DebugShader::new()?,
            lines: Default::default(),
            vertices: Default::default(),
            line_indices: Default::default(),
        })
    }

    pub fn add_line(&mut self, line: Line) {
        self.lines.push(line);
    }

    pub fn clear_lines(&mut self) {
        self.lines.clear()
    }

    pub(in crate) fn render(&mut self, scenes: &SceneContainer) -> RenderPassStatistics {
        let mut statistics = RenderPassStatistics::default();

        self.shader.bind();

        self.vertices.clear();
        self.line_indices.clear();

        let mut i = 0;
        for line in self.lines.iter() {
            let color = line.color.into();
            self.vertices.push(Vertex { position: line.begin, color });
            self.vertices.push(Vertex { position: line.end, color });
            self.line_indices.push([i, i + 1]);
            i += 2;
        }

        self.geometry.set_vertices(&self.vertices);
        self.geometry.set_lines(&self.line_indices);

        unsafe {
            gl::LineWidth(2.0);
            gl::Disable(gl::STENCIL_TEST);
            gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);
            gl::Enable(gl::DEPTH_TEST);
            gl::DepthMask(gl::FALSE);
            gl::Disable(gl::BLEND);
            gl::Disable(gl::CULL_FACE);
        }

        for scene in scenes.iter() {
            // Prepare for render - fill lists of nodes participating in rendering.
            let camera_node = match scene.graph.linear_iter().find(|node| node.is_camera()) {
                Some(camera_node) => camera_node,
                None => continue
            };

            let camera =
                if let Node::Camera(camera) = camera_node {
                    camera
                } else {
                    continue;
                };

            self.shader.set_wvp_matrix(&camera.get_view_projection_matrix());
            self.geometry.draw();
            statistics.draw_calls += 1;
        }


        unsafe {
            gl::DepthMask(gl::TRUE);
        }


        statistics
    }
}