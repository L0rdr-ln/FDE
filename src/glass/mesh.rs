//! Draw a texture as a bent grid: one draw call with a vertex per grid point. Used for wobbly
//! windows. Raw GLES2 inside Smithay's frame; the frame's own state is put back afterwards.

use smithay::backend::renderer::gles::{ffi, GlesError, GlesFrame, GlesRenderer, GlesTexture};

const VERT: &str = r#"
attribute vec2 pos;
attribute vec2 uv;
varying vec2 v_uv;
void main() {
    v_uv = uv;
    gl_Position = vec4(pos, 0.0, 1.0);
}
"#;

const FRAG: &str = r#"
precision mediump float;
varying vec2 v_uv;
uniform sampler2D tex;
uniform float alpha;
uniform float burn;
uniform float inv;
float hash(vec2 p) {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}
float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash(i);
    float b = hash(i + vec2(1.0, 0.0));
    float c = hash(i + vec2(0.0, 1.0));
    float d = hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}
void main() {
    vec4 c = texture2D(tex, v_uv);
    if (burn > 0.0) {
        // a fire front climbs from the bottom: what has burned is gone, its edge glows
        float y = inv > 0.5 ? 1.0 - v_uv.y : v_uv.y;
        float n = 0.6 * noise(v_uv * vec2(9.0, 7.0)) + 0.4 * noise(v_uv * vec2(31.0, 23.0));
        float m = n * 0.62 + (1.0 - y) * 0.38;
        float edge = burn * 1.4 - 0.3 - m;
        if (edge > 0.07) {
            c = vec4(0.0);
        } else if (edge > 0.0) {
            float k = 1.0 - edge / 0.07;
            vec3 ember = mix(vec3(1.0, 0.25, 0.02), vec3(1.0, 0.85, 0.3), k * k);
            c = vec4(ember * k * (0.4 + 0.6 * c.a), k * c.a);
        } else if (edge > -0.10) {
            float k = -edge / 0.10;
            vec3 charred = mix(vec3(0.12, 0.05, 0.02) * c.a, c.rgb, k * k);
            c = vec4(charred + vec3(0.5, 0.12, 0.0) * (1.0 - k) * c.a, c.a);
        }
    }
    gl_FragColor = c * alpha;
}
"#;

#[derive(Clone, Copy)]
pub struct MeshProgram {
    program: ffi::types::GLuint,
    attr_pos: ffi::types::GLint,
    attr_uv: ffi::types::GLint,
    uni_tex: ffi::types::GLint,
    uni_alpha: ffi::types::GLint,
    uni_burn: ffi::types::GLint,
    uni_inv: ffi::types::GLint,
}

unsafe fn compile(gl: &ffi::Gles2, kind: ffi::types::GLenum, src: &str) -> Result<ffi::types::GLuint, GlesError> {
    let shader = gl.CreateShader(kind);
    let ptr = src.as_ptr() as *const ffi::types::GLchar;
    let len = src.len() as ffi::types::GLint;
    gl.ShaderSource(shader, 1, &ptr, &len);
    gl.CompileShader(shader);
    let mut ok = 0;
    gl.GetShaderiv(shader, ffi::COMPILE_STATUS, &mut ok);
    if ok == 0 {
        gl.DeleteShader(shader);
        return Err(GlesError::ShaderCompileError);
    }
    Ok(shader)
}

impl MeshProgram {
    pub fn new(renderer: &mut GlesRenderer) -> Result<MeshProgram, GlesError> {
        renderer.with_context(|gl| unsafe {
            let vs = compile(gl, ffi::VERTEX_SHADER, VERT)?;
            let fs = compile(gl, ffi::FRAGMENT_SHADER, FRAG)?;
            let program = gl.CreateProgram();
            gl.AttachShader(program, vs);
            gl.AttachShader(program, fs);
            gl.LinkProgram(program);
            gl.DetachShader(program, vs);
            gl.DetachShader(program, fs);
            gl.DeleteShader(vs);
            gl.DeleteShader(fs);
            let mut ok = 0;
            gl.GetProgramiv(program, ffi::LINK_STATUS, &mut ok);
            if ok == 0 {
                gl.DeleteProgram(program);
                return Err(GlesError::ProgramLinkError);
            }
            Ok(MeshProgram {
                program,
                attr_pos: gl.GetAttribLocation(program, b"pos\0".as_ptr() as *const _),
                attr_uv: gl.GetAttribLocation(program, b"uv\0".as_ptr() as *const _),
                uni_tex: gl.GetUniformLocation(program, b"tex\0".as_ptr() as *const _),
                uni_alpha: gl.GetUniformLocation(program, b"alpha\0".as_ptr() as *const _),
                uni_burn: gl.GetUniformLocation(program, b"burn\0".as_ptr() as *const _),
                uni_inv: gl.GetUniformLocation(program, b"inv\0".as_ptr() as *const _),
            })
        })?
    }

    /// Draw `tex` over the grid of `n` x `n` points `points` (row by row, in pixels of a target
    /// `size` wide and high, y down). The texture's own corners are the grid's corners.
    pub fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        tex: &GlesTexture,
        target: (i32, i32),
        points: &[(f32, f32)],
        n: usize,
    ) -> Result<(), GlesError> {
        self.draw_with(frame, tex, target, points, n, false, 1.0, (0.0, 0.0, 1.0, 1.0), 0.0)
    }

    /// The same, `alpha` see-through (0 .. 1), showing only the part `uv` = (left, top, right,
    /// bottom) of the texture (each 0 .. 1, y down) stretched over the grid. A torn-off piece of a
    /// window is a 2 x 2 grid with its piece of the texture.
    pub fn draw_part(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        tex: &GlesTexture,
        target: (i32, i32),
        points: &[(f32, f32)],
        n: usize,
        alpha: f32,
        uv: (f32, f32, f32, f32),
    ) -> Result<(), GlesError> {
        self.draw_with(frame, tex, target, points, n, false, alpha, uv, 0.0)
    }

    /// The same, burning away: `burn` (0 .. 1) is how far a fire front has climbed up the picture
    /// from its bottom edge; what it passed is gone and the edge glows.
    pub fn draw_burn(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        tex: &GlesTexture,
        target: (i32, i32),
        points: &[(f32, f32)],
        n: usize,
        alpha: f32,
        burn: f32,
    ) -> Result<(), GlesError> {
        self.draw_with(frame, tex, target, points, n, false, alpha, (0.0, 0.0, 1.0, 1.0), burn)
    }

    /// The same, but the picture under it is multiplied with the texture (night light).
    pub fn draw_multiply(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        tex: &GlesTexture,
        target: (i32, i32),
        points: &[(f32, f32)],
        n: usize,
    ) -> Result<(), GlesError> {
        self.draw_with(frame, tex, target, points, n, true, 1.0, (0.0, 0.0, 1.0, 1.0), 0.0)
    }

    fn draw_with(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        tex: &GlesTexture,
        target: (i32, i32),
        points: &[(f32, f32)],
        n: usize,
        multiply: bool,
        alpha: f32,
        uv: (f32, f32, f32, f32),
        burn: f32,
    ) -> Result<(), GlesError> {
        debug_assert_eq!(points.len(), n * n);
        let mut verts: Vec<f32> = Vec::with_capacity(n * n * 4);
        for i in 0..n {
            for j in 0..n {
                let (x, y) = points[i * n + j];
                let u = uv.0 + (uv.2 - uv.0) * j as f32 / (n - 1) as f32;
                let v_img = uv.1 + (uv.3 - uv.1) * i as f32 / (n - 1) as f32;
                let v = if tex.is_y_inverted() { 1.0 - v_img } else { v_img };
                verts.extend_from_slice(&[x / target.0 as f32 * 2.0 - 1.0, y / target.1 as f32 * 2.0 - 1.0, u, v]);
            }
        }
        let mut idx: Vec<u16> = Vec::with_capacity((n - 1) * (n - 1) * 6);
        for i in 0..n - 1 {
            for j in 0..n - 1 {
                let a = (i * n + j) as u16;
                let (b, c, d) = (a + 1, a + n as u16, a + n as u16 + 1);
                idx.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
        let this = *self;
        let id = tex.tex_id();
        let inverted = tex.is_y_inverted();
        frame.with_context(move |gl| unsafe {
            let mut prev_program = 0;
            gl.GetIntegerv(ffi::CURRENT_PROGRAM, &mut prev_program);
            gl.UseProgram(this.program);
            gl.ActiveTexture(ffi::TEXTURE0);
            gl.BindTexture(ffi::TEXTURE_2D, id);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MIN_FILTER, ffi::LINEAR as i32);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MAG_FILTER, ffi::LINEAR as i32);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_WRAP_S, ffi::CLAMP_TO_EDGE as i32);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_WRAP_T, ffi::CLAMP_TO_EDGE as i32);
            gl.Uniform1i(this.uni_tex, 0);
            gl.Uniform1f(this.uni_alpha, alpha);
            gl.Uniform1f(this.uni_burn, burn);
            gl.Uniform1f(this.uni_inv, if inverted { 1.0 } else { 0.0 });
            gl.Enable(ffi::BLEND);
            if multiply {
                gl.BlendFunc(ffi::DST_COLOR, ffi::ZERO);
            } else {
                gl.BlendFunc(ffi::ONE, ffi::ONE_MINUS_SRC_ALPHA);
            }
            gl.BindBuffer(ffi::ARRAY_BUFFER, 0);
            gl.BindBuffer(ffi::ELEMENT_ARRAY_BUFFER, 0);
            let stride = (4 * std::mem::size_of::<f32>()) as i32;
            gl.EnableVertexAttribArray(this.attr_pos as u32);
            gl.VertexAttribPointer(this.attr_pos as u32, 2, ffi::FLOAT, ffi::FALSE, stride, verts.as_ptr() as *const _);
            gl.EnableVertexAttribArray(this.attr_uv as u32);
            gl.VertexAttribPointer(this.attr_uv as u32, 2, ffi::FLOAT, ffi::FALSE, stride, verts.as_ptr().add(2) as *const _);
            // Smithay leaves instancing divisors on its own attribute slots
            gl.VertexAttribDivisor(this.attr_pos as u32, 0);
            gl.VertexAttribDivisor(this.attr_uv as u32, 0);
            gl.DrawElements(ffi::TRIANGLES, idx.len() as i32, ffi::UNSIGNED_SHORT, idx.as_ptr() as *const _);
            gl.DisableVertexAttribArray(this.attr_pos as u32);
            gl.DisableVertexAttribArray(this.attr_uv as u32);
            gl.BindTexture(ffi::TEXTURE_2D, 0);
            gl.BlendFunc(ffi::ONE, ffi::ONE_MINUS_SRC_ALPHA);
            gl.UseProgram(prev_program as u32);
        })
    }
}
