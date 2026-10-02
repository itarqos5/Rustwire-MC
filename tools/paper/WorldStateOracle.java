// Original API-only synthetic wire oracle. Does not start a server or reproduce game code.
import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

public final class WorldStateOracle {
    static int protocol;
    static Object registry;
    static final PrintStream OUT = System.out;
    static Class<?> cls(String... names) throws Exception {
        for (String name : names) try { return Class.forName(name); } catch (ClassNotFoundException ignored) {}
        throw new ClassNotFoundException(Arrays.toString(names));
    }
    static final class Wire extends ByteArrayOutputStream {
        Wire b(int n) { write(n); return this; }
        Wire vi(int n) { while ((n & ~127) != 0) { b((n & 127) | 128); n >>>= 7; } return b(n); }
        Wire vl(long n) { while ((n & ~127L) != 0) { b(((int)n & 127) | 128); n >>>= 7; } return b((int)n); }
        Wire i64(long n) { for (int i=7;i>=0;i--) b((int)(n >>> (i*8))); return this; }
        Wire f(float n) { int bits=Float.floatToRawIntBits(n);for(int i=3;i>=0;i--)b(bits >>> (i*8));return this; }
        Wire d(double n) { return i64(Double.doubleToRawLongBits(n)); }
        Wire s(String s) { byte[] b=s.getBytes(StandardCharsets.UTF_8); vi(b.length);writeBytes(b);return this; }
    }
    static Object buffer(byte[] payload) throws Exception {
        Class<?> bytebuf=cls("io.netty.buffer.ByteBuf");
        Object raw=cls("io.netty.buffer.Unpooled").getMethod("wrappedBuffer",byte[].class).invoke(null,(Object)payload);
        if(protocol<766)return cls("net.minecraft.network.PacketDataSerializer").getConstructor(bytebuf).newInstance(raw);
        return cls("net.minecraft.network.RegistryFriendlyByteBuf").getConstructor(bytebuf,cls("net.minecraft.core.RegistryAccess")).newInstance(raw,registry);
    }
    static void setup() throws Exception {
        cls("net.minecraft.SharedConstants").getMethod(protocol<766?"a":"tryDetectVersion").invoke(null);
        cls("net.minecraft.server.Bootstrap","net.minecraft.server.DispenserRegistry").getMethod(protocol<766?"a":"bootStrap").invoke(null);
        if(protocol<766)return;
        Class<?> access=cls("net.minecraft.core.RegistryAccess");
        registry=access.getField("EMPTY").get(null);
        if(protocol<775)return;
        Class<?> key=cls("net.minecraft.resources.ResourceKey"),life=cls("com.mojang.serialization.Lifecycle"),info=cls("net.minecraft.core.RegistrationInfo"),mapped=cls("net.minecraft.core.MappedRegistry");
        Object registryKey=cls("net.minecraft.core.registries.Registries").getField("WORLD_CLOCK").get(null);
        Object clocks=mapped.getConstructor(key,life).newInstance(registryKey,life.getMethod("stable").invoke(null));
        Class<?> id=cls("net.minecraft.resources.Identifier","net.minecraft.resources.ResourceLocation");
        for(int n=0;n<3;n++) {
            Object name=id.getMethod("parse",String.class).invoke(null,"rustwire:clock_"+n);
            Object k=key.getMethod("create",key,id).invoke(null,registryKey,name);
            mapped.getMethod("register",key,Object.class,info).invoke(clocks,k,cls("net.minecraft.world.clock.WorldClock").getConstructor().newInstance(),info.getField("BUILT_IN").get(null));
        }
        mapped.getMethod("freeze").invoke(clocks);
        registry=cls("net.minecraft.core.RegistryAccess$ImmutableRegistryAccess").getConstructor(Map.class).newInstance(Map.of(registryKey,clocks));
    }
    static Class<?> packet(String modern,String legacy)throws Exception {return cls("net.minecraft.network.protocol.game."+(protocol<766?legacy:modern));}
    static String error(Throwable e) { while(e instanceof InvocationTargetException)e=((InvocationTargetException)e).getCause(); return e.getClass().getSimpleName()+":"+e.getMessage(); }
    static void check(String label, Class<?> type, Wire wire, boolean mustEqual) throws Exception {
        byte[] input=wire.toByteArray();Object read=buffer(input), value=null, codec=null;
        try {
            if(protocol<766) {
                Constructor<?> k=type.getDeclaredConstructor(read.getClass());k.setAccessible(true);value=k.newInstance(read);
            } else {
                codec=type.getField("STREAM_CODEC").get(null);
                value=cls("net.minecraft.network.codec.StreamCodec").getMethod("decode",Object.class).invoke(codec,read);
            }
            int remaining=(Integer)read.getClass().getMethod("readableBytes").invoke(read);
            Object write=buffer(new byte[0]);
            // wrapped zero-byte buffers are fixed-size: allocate an expandable buffer instead.
            Object raw=cls("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null);
            if(protocol<766)write=read.getClass().getConstructor(cls("io.netty.buffer.ByteBuf")).newInstance(raw);
            else write=read.getClass().getConstructor(cls("io.netty.buffer.ByteBuf"),cls("net.minecraft.core.RegistryAccess")).newInstance(raw,registry);
            if(protocol<766)type.getMethod("a",read.getClass()).invoke(value,write);
            else cls("net.minecraft.network.codec.StreamCodec").getMethod("encode",Object.class,Object.class).invoke(codec,write,value);
            int n=(Integer)write.getClass().getMethod("readableBytes").invoke(write);byte[] output=new byte[n];write.getClass().getMethod("readBytes",byte[].class).invoke(write,(Object)output);
            boolean equal=Arrays.equals(input,output)&&remaining==0;
            OUT.println(label+"\t"+HexFormat.of().formatHex(input)+"\t"+HexFormat.of().formatHex(output)+"\tremaining="+remaining);
            if(mustEqual&&!equal)throw new AssertionError(label+" serializer mismatch");
        } catch (InvocationTargetException e) {
            OUT.println(label+"\t"+HexFormat.of().formatHex(input)+"\tERROR "+error(e));
            if(mustEqual)throw e;
        }
    }
    public static void main(String[] args)throws Exception {
        protocol=Integer.parseInt(args[0]);setup();
        Class<?> game=packet("ClientboundGameEventPacket","PacketPlayOutGameStateChange"),time=packet("ClientboundSetTimePacket","PacketPlayOutUpdateTime"),spawn=packet("ClientboundSetDefaultSpawnPositionPacket","PacketPlayOutSpawnPosition"),difficulty=packet("ClientboundChangeDifficultyPacket","PacketPlayOutServerDifficulty");
        for(int reason=0;reason<16;reason++)check("game_reason_"+reason,game,new Wire().b(reason).f(-3.5f),reason<=(protocol==763?11:protocol==764?12:13));
        for(float n:new float[]{Float.NaN,Float.POSITIVE_INFINITY,Float.NEGATIVE_INFINITY,-0.0f})check("game_scalar_"+Float.floatToRawIntBits(n),game,new Wire().b(7).f(n),true);
        for(int value:new int[]{0,1,2,3,4,127,255,-1,Integer.MIN_VALUE,Integer.MAX_VALUE}) {
            if(protocol<771 && value<0 || protocol<771 && value>255)continue;
            check("difficulty_"+value,difficulty,(protocol<771?new Wire().b(value):new Wire().vi(value)).b(1),value>=0&&value<4);
        }
        Wire normal=new Wire().i64(-123456789L);
        if(protocol<775){normal.i64(Long.MIN_VALUE);if(protocol>=768)normal.b(1);}
        else normal.vi(1).vi(0).vl(-1).f(0.25f).f(-2.5f);
        check("time_golden",time,normal,true);
        if(protocol>=775){check("time_float",time,new Wire().i64(0).vi(1).vi(1).vl(Long.MIN_VALUE).f(Float.intBitsToFloat(0x7fc01234)).f(Float.NEGATIVE_INFINITY),true);check("time_multi",time,new Wire().i64(-123456789L).vi(2).vi(0).vl(-1).f(0.25f).f(-2.5f).vi(2).vl(Long.MAX_VALUE).f(-0.0f).f(Float.POSITIVE_INFINITY),false);check("time_duplicate",time,new Wire().i64(1).vi(2).vi(0).vl(1).f(0).f(1).vi(0).vl(2).f(0).f(1),false);check("time_negative_id",time,new Wire().i64(1).vi(1).vi(-1).vl(1).f(0).f(1),false);}
        for(float angle:new float[]{-123.5f,Float.NaN,Float.POSITIVE_INFINITY}) {
            Wire pos=new Wire();if(protocol>=773)pos.s("minecraft:overworld");pos.i64(0x80000020000007ffL).f(angle);if(protocol>=773)pos.f(-91.5f);check("spawn_"+Float.floatToRawIntBits(angle),spawn,pos,true);
        }
        if(protocol>=773)for(String id:new String[]{"",":","..:x","UPPER:x","minecraft:"}){Wire p=new Wire().s(id).i64(0).f(0).f(0);check("spawn_id_"+id,spawn,p,false);}
        check("border_initialize",packet("ClientboundInitializeBorderPacket","ClientboundInitializeBorderPacket"),new Wire().d(-1.25).d(2.5).d(-3.5).d(4.75).vl(1L<<40).vi(-1).vi(Integer.MIN_VALUE).vi(Integer.MAX_VALUE),true);
        check("border_center",packet("ClientboundSetBorderCenterPacket","ClientboundSetBorderCenterPacket"),new Wire().d(Double.NaN).d(Double.NEGATIVE_INFINITY),true);
        check("border_lerp",packet("ClientboundSetBorderLerpSizePacket","ClientboundSetBorderLerpSizePacket"),new Wire().d(-1.25).d(Double.POSITIVE_INFINITY).vl(Long.MIN_VALUE),true);
        check("border_size",packet("ClientboundSetBorderSizePacket","ClientboundSetBorderSizePacket"),new Wire().d(-3.5),true);
        check("border_warning_delay",packet("ClientboundSetBorderWarningDelayPacket","ClientboundSetBorderWarningDelayPacket"),new Wire().vi(Integer.MIN_VALUE),true);
        check("border_warning_distance",packet("ClientboundSetBorderWarningDistancePacket","ClientboundSetBorderWarningDistancePacket"),new Wire().vi(-1),true);
        check("block_ack",packet("ClientboundBlockChangedAckPacket","ClientboundBlockChangedAckPacket"),new Wire().vi(Integer.MIN_VALUE),true);
        if(protocol>=769)check("player_loaded",packet("ServerboundPlayerLoadedPacket","ServerboundPlayerLoadedPacket"),new Wire(),true);
    }
}
