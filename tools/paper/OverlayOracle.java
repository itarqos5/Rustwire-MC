// Original synthetic protocol-fixture harness using installed release APIs.
// No game implementation code, server launch, worlds, or network connections.
// Compile with a JDK; run with one prepared Paper jar and its libraries.
import java.io.ByteArrayOutputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
public final class OverlayOracle {
    // Bootstrap can redirect System.out through asynchronous logging. Keep fixtures deterministic.
    static final PrintStream report = new PrintStream(new FileOutputStream(FileDescriptor.out), true);
    static Class<?> bytebuf, buffer;
    static boolean modern;
    static Object registry;
    static Class<?> find(String... names) throws Exception {
        for (String name:names) try { return Class.forName(name); } catch(ClassNotFoundException ignored) {}
        throw new ClassNotFoundException(Arrays.toString(names));
    }
    static Method method(Class<?> owner,Class<?> result,Class<?>... args) {
        for(Method m:owner.getDeclaredMethods()) if(m.getReturnType()==result && Arrays.equals(m.getParameterTypes(),args)) { m.setAccessible(true);return m; }
        throw new IllegalStateException(owner+" missing "+result+Arrays.toString(args));
    }
    static void varint(ByteArrayOutputStream b,int n) { do { int x=n&127;n>>>=7;b.write(x|(n==0?0:128)); } while(n!=0); }
    static byte[] component(int protocol,String s) {
        ByteArrayOutputStream b=new ByteArrayOutputStream();byte[] bytes=(protocol<765?"{\"text\":\""+s+"\"}":s).getBytes(StandardCharsets.UTF_8);
        if(protocol<765)varint(b,bytes.length);else { b.write(8);b.write(bytes.length>>>8);b.write(bytes.length); }b.writeBytes(bytes);return b.toByteArray();
    }
    static byte[] boss(int protocol,int action,int bits,int color,int division,int flags) {
        ByteArrayOutputStream b=new ByteArrayOutputStream();for(int i=0;i<16;i++)b.write(i);varint(b,action);
        if(action==0||action==3)b.writeBytes(component(protocol,"x"));
        if(action==0||action==2)for(int i=3;i>=0;i--)b.write(bits>>>(8*i));
        if(action==0||action==4) { varint(b,color);varint(b,division); }
        if(action==0||action==5)b.write(flags);return b.toByteArray();
    }
    static Object make(byte[] input) throws Exception {
        Object raw=input==null?Class.forName("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null):Class.forName("io.netty.buffer.Unpooled").getMethod("wrappedBuffer",byte[].class).invoke(null,(Object)input);
        if(modern)return buffer.getConstructor(bytebuf,Class.forName("net.minecraft.core.RegistryAccess")).newInstance(raw,registry);
        return buffer.getConstructor(bytebuf).newInstance(raw);
    }
    static Object decode(Class<?> packet,byte[] bytes) throws Exception {
        Object b=make(bytes),result;
        if(modern) { Object codec=packet.getField("STREAM_CODEC").get(null);result=Class.forName("net.minecraft.network.codec.StreamCodec").getMethod("decode",Object.class).invoke(codec,b); }
        else { Constructor<?> constructor=packet.getDeclaredConstructor(buffer);constructor.setAccessible(true);result=constructor.newInstance(b); }
        if((int)bytebuf.getMethod("readableBytes").invoke(b)!=0)throw new AssertionError("trailing bytes");return result;
    }
    static byte[] encode(Class<?> packet,Object value) throws Exception {
        Object b=make(null);
        if(modern) { Object codec=packet.getField("STREAM_CODEC").get(null);Class.forName("net.minecraft.network.codec.StreamCodec").getMethod("encode",Object.class,Object.class).invoke(codec,b,value); }
        else method(packet,void.class,buffer).invoke(value,b);
        byte[] out=new byte[(int)bytebuf.getMethod("readableBytes").invoke(b)];bytebuf.getMethod("readBytes",byte[].class).invoke(b,(Object)out);return out;
    }
    static void fixture(Class<?> packet,String label,byte[] bytes,byte[] expected) throws Exception {
        byte[] actual=encode(packet,decode(packet,bytes));if(!Arrays.equals(actual,expected))throw new AssertionError(label+" expected="+HexFormat.of().formatHex(expected)+" actual="+HexFormat.of().formatHex(actual));
        report.println(label+"="+HexFormat.of().formatHex(actual));
    }
    static void invalid(Class<?> packet,String label,byte[] bytes) throws Exception {
        try { decode(packet,bytes);throw new AssertionError("accepted "+label); }
        catch(InvocationTargetException e) {report.println(label+"=rejected:"+e.getCause().getClass().getSimpleName());}
    }
    public static void main(String[] args) throws Exception {
        Class<?> constants=Class.forName("net.minecraft.SharedConstants");
        try { constants.getMethod("tryDetectVersion").invoke(null); }
        catch(NoSuchMethodException e) { constants.getMethod("a").invoke(null); }
        Class<?> bootstrap=find("net.minecraft.server.Bootstrap","net.minecraft.server.DispenserRegistry");
        try { bootstrap.getMethod("bootStrap").invoke(null); }
        catch(NoSuchMethodException e) { bootstrap.getMethod("a").invoke(null); }
        int protocol=Integer.parseInt(args[0]);modern=protocol>=766;bytebuf=Class.forName("io.netty.buffer.ByteBuf");
        buffer=find(modern?"net.minecraft.network.RegistryFriendlyByteBuf":"net.minecraft.network.PacketDataSerializer");
        if(modern) registry=Class.forName("net.minecraft.core.RegistryAccess").getField("EMPTY").get(null);
        Class<?> boss=find("net.minecraft.network.protocol.game.ClientboundBossEventPacket","net.minecraft.network.protocol.game.PacketPlayOutBoss");
        Class<?> list=find("net.minecraft.network.protocol.game.ClientboundTabListPacket","net.minecraft.network.protocol.game.PacketPlayOutPlayerListHeaderFooter");
        report.println("protocol="+protocol);
        for(int action=0;action<=5;action++) { byte[] bytes=boss(protocol,action,0x3fc00000,6,4,7);fixture(boss,"action_"+action,bytes,bytes); }
        ByteArrayOutputStream listBytes=new ByteArrayOutputStream();listBytes.writeBytes(component(protocol,"x"));listBytes.writeBytes(component(protocol,"y"));byte[] listInput=listBytes.toByteArray();fixture(list,"header_footer",listInput,listInput);
        for(int color=0;color<=6;color++)for(int division=0;division<=4;division++) {byte[] bytes=boss(protocol,4,0,color,division,0);fixture(boss,"style_"+color+"_"+division,bytes,bytes);}
        for(int bits:new int[]{0x80000000,0xbf800000,0x7f800000,0xff800000,0x7fc01234}) {byte[] bytes=boss(protocol,2,bits,0,0,0);fixture(boss,"health_"+Integer.toHexString(bits),bytes,bytes);}
        for(int flags:new int[]{0,1,2,4,7,8,128,255})fixture(boss,"flags_"+flags,boss(protocol,5,0,0,0,flags),boss(protocol,5,0,0,0,flags&7));
        for(int action:new int[]{-1,6,Integer.MAX_VALUE})invalid(boss,"invalid_action_"+action,boss(protocol,action,0,0,0,0));
        for(int color:new int[]{-1,7,Integer.MAX_VALUE})invalid(boss,"invalid_color_"+color,boss(protocol,4,0,color,0,0));
        for(int division:new int[]{-1,5,Integer.MAX_VALUE})invalid(boss,"invalid_division_"+division,boss(protocol,4,0,0,division,0));
    }
}
