//===============================================================================
// #36	Joscelind Wadcock 
//===============================================================================

class  WCWadcock extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardWadcockTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wadcocksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWadcockBigTexture FILE=TEXTURES\menu\Folio\Cards\Wadcockbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Joscelind Wadcock";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=36
    skin(0)=Texture'HProps.skins.WizardCardWadcockTex0'
	textureBig=Texture'WizCardWadcockBigTexture'
	strDescriptionId="WizCard_0032"
}
